use std::path::Path;
use std::time::Instant;

use crate::audit;
use crate::canon;
use crate::clippy_cmd;
use crate::coverage;
use crate::deny;
use crate::dep_age_gate;
use crate::doc_cmd;
use crate::dupes;
use crate::fmt_cmd;
use crate::records;
use crate::step::Step;
use crate::test_cmd;
#[cfg(feature = "frontend")]
use crate::{frontend_check, frontend_dupes, frontend_fmt, frontend_test};

/// Width of the step name column (including dots).
const STEP_NAME_WIDTH: usize = 14;

/// Build the ordered list of validation steps.
///
/// Both the per-step index and the total derive from this one
/// list, so the `[N/M]` numbering follows whichever gates are
/// compiled in. A unit test resolves every `cmd` through the
/// real CLI, so a renamed subcommand cannot leave the
/// `iterate with` hint pointing at a command that no longer
/// exists.
///
/// Ordering rationale: Dep-age first, then cheap static gates,
/// then the expensive dynamic gates -- see [`validate`].
fn steps(check: bool) -> Vec<Step> {
    let mut steps = vec![
        // Fail fast on a within-cooldown dependency before
        // compiling it.
        dep_age_gate::step(),
        // Cheap static gates first ...
        fmt_cmd::step(check),
        // Reads markdown only, so it needs no compilation and
        // sits ahead of every gate that does.
        canon::step(),
        records::step(),
        dupes::step(),
        clippy_cmd::step(),
        // Reuses the metadata Clippy just produced, so both rustdoc
        // passes cost about a second warm.
        doc_cmd::step(),
        // Offline supply-chain gate -- see Audit (network) at the
        // end. Placed among the static gates because it compiles
        // nothing and reaches no network.
        deny::step(),
    ];
    // Three static frontend gates, then the dynamic frontend test.
    #[cfg(feature = "frontend")]
    steps.extend([
        frontend_fmt::step(check),
        frontend_check::step(),
        frontend_dupes::step(),
        frontend_test::step(),
    ]);
    // ... expensive dynamic gates last.
    steps.extend([test_cmd::step(), coverage::step(), audit::step()]);
    steps
}

/// Run all validation steps with concise stepwise
/// output.
///
/// Dep-age runs first so a dependency adopted within the
/// cooldown fails the gate *before* the compile steps
/// (Clippy, Test, Coverage) build -- and run its build
/// script -- on the too-new crate. It is free when the
/// lockfiles are unchanged (no network, ~0.1s), so on the
/// common no-dep-change commit its early placement costs
/// nothing; it only reaches the network on a commit that
/// actually adopts a dependency, and even then a
/// connectivity failure degrades to a warning (a genuine
/// cooldown breach still fails; a transient outage does not),
/// except under CI or `RUSTBASE_DEP_AGE_STRICT=1`, where it
/// fails -- see `dep_age_gate`.
///
/// After Dep-age, steps run cheap static gates first and the
/// expensive dynamic gates (Test, Coverage) last, so a fast
/// check's failure is not gated behind the multi-minute
/// instrumented Coverage run. Fmt leads the static gates
/// because it rewrites whitespace that later checks read. The
/// Audit step is network-dependent, so it runs last (after
/// Coverage) with the same degrade-to-warning treatment as
/// Dep-age -- a positive vulnerability finding still fails the
/// gate, but a transient outage does not.
///
/// `check` selects fmt's mode: `false` (default) auto-fixes
/// formatting in place; `true` runs the read-only
/// `fmt --check` for CI or before partial staging, where
/// an in-place rewrite would sweep unrelated drift into the
/// working tree.
pub fn validate(check: bool) -> Result<(), String> {
    let overall_start = Instant::now();

    frontend_feature_check(
        cfg!(feature = "frontend"),
        Path::new("frontend/package.json").exists(),
    )?;
    let steps = steps(check);
    let total = steps.len();
    for (i, step) in steps.into_iter().enumerate() {
        run_step(i + 1, total, step)?;
    }

    println!("Validate OK ({})", elapsed_str(overall_start));
    Ok(())
}

/// Refuse a frontend on disk when the `frontend` feature is off:
/// its gates are then compiled out, and a run with none of them
/// would read as a pass.
fn frontend_feature_check(
    feature_on: bool,
    has_frontend: bool,
) -> Result<(), String> {
    if has_frontend && !feature_on {
        return Err("frontend/ exists but xtask's `frontend` feature is \
             off, so validate would run none of its gates -- add \
             \"frontend\" to `default` in xtask/Cargo.toml, or delete \
             frontend/"
            .into());
    }
    Ok(())
}

/// Run a single step, printing the `[N/M]` result line.
///
/// `total` is the step count, so the `[N/M]` denominator tracks
/// the list automatically. On failure the
/// step's `cmd` is printed as an iterate-with hint so the user
/// re-runs the single failing gate (seconds) instead of the
/// whole pipeline (minutes).
fn run_step(n: usize, total: usize, step: Step) -> Result<(), String> {
    let (name, cmd) = (step.name(), step.cmd());
    let start = Instant::now();
    match step.run() {
        Ok(detail) => {
            let time = elapsed_str(start);
            let full = if detail.is_empty() {
                time
            } else {
                format!("{detail}, {time}")
            };
            step_output(n, total, name, "OK", &full);
            Ok(())
        }
        Err(e) => {
            step_output(n, total, name, "FAILED", "");
            eprintln!("  -> iterate with: cargo xtask {cmd}");
            Err(e)
        }
    }
}

/// Format a validation step result line as a string.
///
/// Returns: `[1/5] Fmt........... OK (0.3s)`
fn format_step(
    step: usize,
    total: usize,
    name: &str,
    status: &str,
    detail: &str,
) -> String {
    let dots = ".".repeat(STEP_NAME_WIDTH.saturating_sub(name.len()));
    if detail.is_empty() {
        format!("[{step}/{total}] {name}{dots} {status}")
    } else {
        format!(
            "[{step}/{total}] {name}{dots} {status} \
             ({detail})"
        )
    }
}

/// Print a validation step result line to stdout.
///
/// Produces: `[1/5] Fmt........... OK (0.3s)`
fn step_output(
    step: usize,
    total: usize,
    name: &str,
    status: &str,
    detail: &str,
) {
    println!("{}", format_step(step, total, name, status, detail));
}

/// Format elapsed time as a human-readable string.
fn elapsed_str(start: Instant) -> String {
    let secs = start.elapsed().as_secs_f64();
    format!("{secs:.1}s")
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn every_step_hint_resolves_to_a_subcommand() {
        // Each Step's `cmd` is printed as the
        // `iterate with: cargo xtask <cmd>` hint on failure.
        // Resolve every one through the actual CLI so a renamed
        // or removed subcommand cannot leave the hint pointing at
        // a command that does not exist. `check` does not affect
        // the cmd strings, so either value exercises them all.
        let cli = crate::Cli::command();
        for step in steps(false) {
            assert!(
                cli.find_subcommand(step.cmd()).is_some(),
                "validate step {:?} hints `cargo xtask {}`, \
                 which is not a real subcommand",
                step.name(),
                step.cmd(),
            );
        }
    }

    #[test]
    fn steps_list_is_non_empty() {
        // The `[N/M]` denominator derives from this length;
        // an empty list would silently disable the whole gate.
        assert!(!steps(false).is_empty(), "validate has no steps");
    }

    #[test]
    fn frontend_steps_follow_the_feature() {
        // The frontend gates are in the list exactly when the
        // feature compiles them in, so the `[N/M]` total counts
        // only gates that exist.
        let has_frontend = steps(false)
            .iter()
            .any(|s| s.cmd().starts_with("frontend-"));
        assert_eq!(has_frontend, cfg!(feature = "frontend"));
    }

    #[test]
    fn frontend_on_disk_needs_the_frontend_feature() {
        // With the feature off its gates are compiled out, so a
        // frontend on disk would pass validate unchecked.
        let err = frontend_feature_check(false, true).unwrap_err();
        assert!(err.contains("`frontend` feature"), "{err}");
        assert!(frontend_feature_check(true, true).is_ok());
        assert!(frontend_feature_check(true, false).is_ok());
        assert!(frontend_feature_check(false, false).is_ok());
    }

    #[test]
    fn format_step_with_detail() {
        assert_eq!(
            format_step(1, 5, "Fmt", "OK", "0.3s"),
            "[1/5] Fmt........... OK (0.3s)"
        );
    }

    #[test]
    fn format_step_without_detail() {
        assert_eq!(
            format_step(2, 5, "Clippy", "FAILED", ""),
            "[2/5] Clippy........ FAILED"
        );
    }

    #[test]
    fn format_step_long_name_no_overflow() {
        let result = format_step(1, 1, "VeryLongStepName", "OK", "");
        assert_eq!(result, "[1/1] VeryLongStepName OK");
    }

    #[test]
    fn elapsed_str_format() {
        let start = Instant::now();
        let result = elapsed_str(start);
        assert!(result.ends_with('s'), "should end with 's': {result}");
        assert!(result.contains('.'), "should have decimal: {result}");
    }
}
