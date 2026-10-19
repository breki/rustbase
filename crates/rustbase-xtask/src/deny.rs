//! The licence gate.
//!
//! Runs `cargo deny check licenses bans sources` over `Cargo.lock`,
//! configured by `deny.toml` at the workspace root.
//!
//! **Offline, and that is the point.** These three checks read the
//! lockfile and the crate metadata already on disk; none of them
//! reaches the network. So unlike [`crate::audit`] this can run on
//! every push without failing for reasons unrelated to the commit.
//! Advisories stay with `cargo-audit`, which does need the RUSTSEC
//! database.
//!
//! A missing `cargo-deny` is an **error**, and this gate takes no
//! [`crate::Strictness`]. `audit`'s leniency buys an offline laptop
//! the ability to finish `validate`, at the cost of the advisory DB
//! going unconsulted. Here there is nothing to trade: the check needs
//! no network, so a machine that cannot run it is simply missing a
//! tool, and saying so is more use than passing quietly.

use crate::process::{self, Captured};
use crate::{Config, is_missing_subcommand};

/// What `cargo deny` reported.
///
/// Three named outcomes rather than a bool and a string, because a
/// missing tool must never be spelled as a failure that happens to
/// carry install text in its detail field.
#[derive(Debug, PartialEq, Eq)]
enum DenyResult {
    /// Every check passed.
    Passed,
    /// A check failed; the string is the tool's own output, which
    /// can be empty when cargo-deny fails and prints nothing.
    Failed(String),
    /// `cargo-deny` is not installed.
    ToolMissing,
}

/// What to tell the operator when cargo-deny is not installed.
const MISSING_TOOL: &str =
    "cargo-deny is not installed: cargo install --locked cargo-deny";

/// The `cargo` arguments the gate runs.
const ARGS: &[&str] = &[
    "deny",
    // `--offline` makes "reads what is already on disk" true rather
    // than asserted: without it cargo-deny resolves and *fetches*
    // the tree.
    "--offline",
    "--locked",
    "--all-features",
    "check",
    "licenses",
    "bans",
    "sources",
];

/// Runs the gate in the workspace root.
///
/// # Errors
///
/// The tool's output when a check fails, or how to install it when
/// it is missing.
pub fn check(cfg: &Config) -> Result<(), String> {
    outcome(&process::cargo(&cfg.root, ARGS)?)
}

/// Turn a finished run into the gate's result.
fn outcome(out: &Captured) -> Result<(), String> {
    // Both streams. cargo-deny puts its diagnostics on stderr, but a
    // malformed `deny.toml` or another version's wording can land on
    // stdout.
    let both = format!("{}\n{}", out.stderr.trim(), out.stdout.trim());
    match failure(classify(out.ok, &both), &out.status) {
        None => Ok(()),
        Some(e) => Err(e),
    }
}

/// Classifies a `cargo deny` run.
fn classify(status_ok: bool, output: &str) -> DenyResult {
    // Checked before `status_ok`, because a missing tool must never
    // come back as a pass: there is no network to be down here.
    if is_missing_subcommand(output) {
        return DenyResult::ToolMissing;
    }
    if status_ok {
        DenyResult::Passed
    } else {
        DenyResult::Failed(output.trim().to_owned())
    }
}

/// Turns an outcome into the error the caller returns, or `None`
/// when the gate passed. A failure that printed nothing still
/// names the exit status, the one fact available.
fn failure(result: DenyResult, status: &str) -> Option<String> {
    match result {
        DenyResult::Passed => None,
        DenyResult::Failed(detail) if detail.is_empty() => {
            Some(format!("cargo deny failed ({status}) and printed nothing"))
        }
        DenyResult::Failed(detail) => Some(detail),
        DenyResult::ToolMissing => Some(MISSING_TOOL.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(ok: bool, stdout: &str, stderr: &str) -> Result<(), String> {
        outcome(&Captured {
            ok,
            status: "exit status: 2".into(),
            stdout: stdout.into(),
            stderr: stderr.into(),
        })
    }

    #[test]
    fn a_passing_run_is_ok() {
        assert_eq!(classify(true, ""), DenyResult::Passed);
        assert_eq!(run(true, "", "ok"), Ok(()));
    }

    #[test]
    fn a_failing_check_carries_the_tools_output() {
        let e = run(false, "", "error[L001]: failed to satisfy license")
            .unwrap_err();
        assert!(e.contains("L001"), "{e}");
    }

    #[test]
    fn output_on_stdout_is_not_lost() {
        let e = run(false, "malformed deny.toml", "").unwrap_err();
        assert!(e.contains("malformed deny.toml"), "{e}");
    }

    #[test]
    fn a_missing_tool_says_how_to_install_it() {
        // cargo's own wording names the subcommand and nothing else.
        let e = run(false, "", "error: no such command: `deny`").unwrap_err();
        assert!(e.contains("cargo install"), "{e}");
        assert!(e.contains("cargo-deny"), "{e}");
    }

    #[test]
    fn a_missing_tool_is_never_reported_as_a_pass() {
        // Even when the exit status says success.
        assert_eq!(
            classify(true, "error: no such command: `deny`"),
            DenyResult::ToolMissing
        );
    }

    #[test]
    fn a_silent_failure_still_names_the_exit_status() {
        let e = run(false, "", "").unwrap_err();
        assert!(e.contains("exit status: 2"), "{e}");
        assert!(e.contains("printed nothing"), "{e}");
    }
}
