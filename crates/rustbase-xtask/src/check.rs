//! The type-check gate: `cargo check` over every target, reported
//! as the compilation errors alone.

use crate::process::{self, Captured};
use crate::text::stderr_tail;
use crate::{Config, Failure};

/// Arguments for the `cargo check` invocation.
///
/// `--all-targets` is load-bearing: without it cargo compiles only
/// the library and binary targets, so a change that breaks a test
/// call site still passes here and fails later under `test`. A unit
/// test pins the flag.
const CHECK_ARGS: &[&str] = &[
    "check",
    "--workspace",
    "--all-targets",
    "--message-format=short",
];

/// Stderr lines kept when cargo fails with no compilation error to
/// show (a bad manifest, a corrupted `Cargo.lock`, no network).
const STDERR_TAIL_LINES: usize = 20;

/// Type-checks the workspace in its root.
///
/// # Errors
///
/// The compilation errors, or stderr's tail when cargo failed for
/// another reason, or why cargo could not run.
pub fn check(cfg: &Config) -> Result<(), Failure> {
    judge(&process::cargo(&cfg.root, CHECK_ARGS)?)
}

/// Turn a finished `cargo check` into the gate's result.
fn judge(out: &Captured) -> Result<(), Failure> {
    if out.ok {
        return Ok(());
    }
    let errors: Vec<String> = extract_error_lines(&out.stderr)
        .into_iter()
        .map(String::from)
        .collect();
    if errors.is_empty() {
        let tail = stderr_tail(&out.stderr, STDERR_TAIL_LINES);
        return Err(Failure::new(
            "cargo check failed with no compilation error to show",
            tail.into_iter().map(String::from).collect(),
        ));
    }
    Err(Failure::new(
        format!("{} compilation error(s)", errors.len()),
        errors,
    ))
}

/// The compilation errors in `cargo check --message-format=short`
/// stderr. Short-format diagnostics are single lines prefixed by the
/// location (`path:line:col: error[E...]: message`), so the match is
/// on the `: error` separator, plus lines rustc prints at column 0.
/// The summaries are dropped: `aborting due to` (anchored, so a user
/// error mentioning "aborting" survives) and cargo's `could not
/// compile`, which it prints once per failing target and which
/// would otherwise inflate the count.
fn extract_error_lines(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|l| {
            l.contains(": error[")
                || l.contains(": error:")
                || l.starts_with("error[")
                || l.starts_with("error:")
        })
        .filter(|l| !l.starts_with("error: aborting due to"))
        .filter(|l| !l.starts_with("error: could not compile"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(ok: bool, stderr: &str) -> Captured {
        Captured {
            ok,
            status: String::new(),
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }

    #[test]
    fn check_args_compile_all_targets() {
        assert!(
            CHECK_ARGS.contains(&"--all-targets"),
            "check must compile all targets, not just lib+bin"
        );
    }

    #[test]
    fn extracts_short_format_path_prefixed_errors() {
        let stderr = "\
crates/rustbase/src/lib.rs:45:12: error[E0425]: cannot find value `foo`
crates/rustbase/src/lib.rs:50:1: error: expected `;`, found `}`
crates/rustbase/src/lib.rs:9:9: warning: unused variable: `x`
error: aborting due to 2 previous errors";
        let errors = extract_error_lines(stderr);
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("E0425"));
        assert!(errors[1].contains("expected"));
    }

    #[test]
    fn warnings_and_empty_input_give_nothing() {
        for stderr in ["", "warning: unused variable: `x`"] {
            let got = extract_error_lines(stderr);
            assert!(got.is_empty(), "{got:?}");
        }
    }

    #[test]
    fn keeps_user_errors_mentioning_aborting() {
        let stderr = "\
error: aborting: feature X required
error: aborting due to 1 previous error";
        let errors = extract_error_lines(stderr);
        assert_eq!(errors, ["error: aborting: feature X required"]);
    }

    #[test]
    fn one_error_is_counted_once_across_targets() {
        // `--all-targets` makes cargo print `could not compile` for
        // every target the error breaks; counting those turned one
        // error into "4 errors" and crowded out the real one.
        let stderr = "\
crates/a/src/lib.rs:3:5: error[E0425]: cannot find value `x`
error: could not compile `a` (lib) due to 1 previous error
error: could not compile `a` (lib test) due to 1 previous error
error: could not compile `a` (bin \"a\") due to 1 previous error";
        let f = judge(&captured(false, stderr)).unwrap_err();
        assert_eq!(f.summary, "1 compilation error(s)");
        assert_eq!(f.items.len(), 1, "{:?}", f.items);
    }

    #[test]
    fn a_failure_with_no_compile_error_carries_the_stderr_tail() {
        // No rustc error to show (here, the registry is unreachable);
        // the tail goes into the failure itself so the report says
        // why, once.
        let stderr = "    Updating crates.io index\n\
                      warning: spurious network error (3 tries remaining)\n\
                      failed to download `serde v1.0.200`";
        let f = judge(&captured(false, stderr)).unwrap_err();
        assert!(f.summary.contains("no compilation error"), "{}", f.summary);
        assert!(f.items.iter().any(|l| l.contains("failed to download")));
    }

    #[test]
    fn a_passing_run_is_ok() {
        assert_eq!(judge(&captured(true, "warning: x")), Ok(()));
    }
}
