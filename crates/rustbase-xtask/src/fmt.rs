//! The format gate: `cargo fmt`, either rewriting the tree or
//! checking it read-only.

use crate::process::{self, Captured};
use crate::{Config, Failure};

/// Arguments of the read-only check.
const CHECK_ARGS: &[&str] = &["fmt", "--all", "--", "--check"];

/// Formats the workspace in place, streaming cargo's output.
///
/// # Errors
///
/// When `cargo fmt` fails or cannot run.
pub fn fix(cfg: &Config) -> Result<(), Failure> {
    Ok(process::cargo_stream(&cfg.root, &["fmt", "--all"])?)
}

/// Checks formatting without changing the tree.
///
/// # Errors
///
/// The diff `cargo fmt --check` printed, or why it could not run.
pub fn check(cfg: &Config) -> Result<(), Failure> {
    check_with(|| process::cargo(&cfg.root, CHECK_ARGS))
}

/// [`check`] with the cargo spawn injected.
fn check_with(
    cargo: impl FnOnce() -> Result<Captured, String>,
) -> Result<(), Failure> {
    let out = cargo()?;
    if out.ok {
        return Ok(());
    }
    let items = out.stdout.lines().map(String::from).collect();
    Err(Failure::new("formatting issues", items))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(ok: bool, stdout: &str) -> Captured {
        Captured {
            ok,
            status: String::new(),
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    #[test]
    fn unformatted_code_is_reported_with_the_diff() {
        let diff = "Diff in src/lib.rs:1:\n-fn a(){}\n+fn a() {}";
        let f = check_with(|| Ok(captured(false, diff))).unwrap_err();
        assert_eq!(f.summary, "formatting issues");
        assert_eq!(f.items.len(), 3);
        assert_eq!(check_with(|| Ok(captured(true, ""))), Ok(()));
    }

    #[test]
    fn check_fails_on_unformatted_code_until_fix_rewrites_it() {
        // A real one-file crate: rustfmt needs no network or deps.
        let dir = crate::test_repo::empty_dir();
        std::fs::write(
            dir.0.join("Cargo.toml"),
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
             [workspace]\n",
        )
        .unwrap();
        std::fs::create_dir(dir.0.join("src")).unwrap();
        std::fs::write(dir.0.join("src/lib.rs"), "pub fn a(){}\n").unwrap();
        let cfg = Config::for_project("t", dir.0.clone());
        let f = check(&cfg).unwrap_err();
        assert_eq!(f.summary, "formatting issues");
        assert_eq!(fix(&cfg), Ok(()));
        assert_eq!(check(&cfg), Ok(()));
    }

    #[test]
    fn a_cargo_that_cannot_run_is_a_failure() {
        let f = check_with(|| Err("no cargo".into())).unwrap_err();
        assert_eq!(f.summary, "no cargo");
    }
}
