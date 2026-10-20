//! The format gate: `cargo fmt`, either rewriting the tree or
//! checking it read-only.

use crate::process::{self, Captured};
use crate::text::pair_with_locations;
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
    if out.stdout.trim().is_empty() {
        // A parse error or a missing rustfmt prints no diff; the
        // cause is on stderr.
        let none: [&str; 0] = [];
        return Err(Failure::of_run(
            "cargo fmt --check failed with no diff to show",
            none,
            &out,
        ));
    }
    // A file that does not parse puts its error on stderr even when
    // other files produced a diff; formatting will not fix it, so it
    // leads the report.
    let errors = pair_with_locations(&out.stderr, |l| {
        l.starts_with("error") || l.starts_with("Error")
    });
    if errors.is_empty() {
        return Err(Failure::new("formatting issues", out.stdout.lines()));
    }
    Err(Failure::new(
        "files that do not parse, and formatting issues",
        errors.into_iter().chain(out.stdout.lines()),
    ))
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
    fn a_failure_with_no_diff_carries_the_stderr_tail() {
        // A parse error or a missing rustfmt prints no diff; the
        // cause is on stderr.
        let out = Captured {
            ok: false,
            status: String::new(),
            stdout: String::new(),
            stderr: "error: expected one of `;` or `}`\n  --> src/a.rs:3:1"
                .into(),
        };
        let f = check_with(|| Ok(out)).unwrap_err();
        assert!(f.summary.contains("no diff"), "{}", f.summary);
        assert!(f.items.iter().any(|l| l.contains("src/a.rs:3:1")));
    }

    #[test]
    fn a_parse_error_beside_a_diff_is_reported_too() {
        // One crate drifts (a diff on stdout), another does not parse
        // (an error on stderr): formatting will not fix the second.
        let out = Captured {
            ok: false,
            status: String::new(),
            stdout: "Diff in b/src/lib.rs:1:\n-pub fn b(){}\n+pub fn b() {}"
                .into(),
            stderr: "error: this file contains an unclosed delimiter\n  \
                     --> a/src/lib.rs:1:15"
                .into(),
        };
        let f = check_with(|| Ok(out)).unwrap_err();
        assert!(f.items.iter().any(|l| l.contains("unclosed delimiter")));
        assert!(f.items.iter().any(|l| l.contains("Diff in b/src/lib.rs")));
        assert!(f.summary.contains("parse"), "{}", f.summary);
    }

    #[test]
    fn rustfmt_errors_with_a_capital_e_are_reported_too() {
        let out = Captured {
            ok: false,
            status: String::new(),
            stdout: "Diff in a/src/lib.rs:1:".into(),
            stderr: "Error writing files: failed to resolve mod `nope`".into(),
        };
        let f = check_with(|| Ok(out)).unwrap_err();
        assert!(f.items.iter().any(|l| l.contains("failed to resolve mod")));
    }

    #[test]
    fn a_cargo_that_cannot_run_is_a_failure() {
        let f = check_with(|| Err("no cargo".into())).unwrap_err();
        assert_eq!(f.summary, "no cargo");
    }
}
