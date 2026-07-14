use std::time::Instant;

use crate::audit;
use crate::clippy_cmd;
use crate::coverage;
use crate::dupes;
use crate::fmt_cmd;
use crate::frontend_check;
use crate::frontend_dupes;
use crate::frontend_fmt;
use crate::frontend_test;
use crate::helpers::{elapsed_str, step_output};
use crate::test_cmd;

/// Total number of validation steps.
const TOTAL_STEPS: usize = 10;

/// Run all validation steps with concise stepwise
/// output.
///
/// Steps run cheap static gates first and the expensive
/// dynamic gates (Test, Coverage) last, so a fast check's
/// failure is not gated behind the multi-minute
/// instrumented Coverage run. Fmt stays first because it
/// rewrites whitespace that later checks read. The Audit
/// step is network-dependent, so it runs last (after
/// Coverage) and degrades a connectivity failure to a
/// warning -- a positive vulnerability finding still fails
/// the gate, but a transient outage does not.
///
/// `check` selects fmt's mode: `false` (default) auto-fixes
/// formatting in place; `true` runs the read-only
/// `fmt --check` for CI or before partial staging, where
/// an in-place rewrite would sweep unrelated drift into the
/// working tree.
pub fn validate(check: bool) -> Result<(), String> {
    let overall_start = Instant::now();

    // Cheap static gates first ...
    run_step(1, "Fmt", "fmt", || run_fmt(check))?;
    run_step(2, "Duplication", "dupes", run_duplication)?;
    run_step(3, "Clippy", "clippy", run_clippy)?;
    run_step(4, "Frontend fmt", "frontend-fmt", || {
        run_frontend_fmt(check)
    })?;
    run_step(5, "Frontend check", "frontend-check", run_frontend_check)?;
    run_step(6, "Frontend dupes", "frontend-dupes", run_frontend_dupes)?;

    // ... expensive dynamic gates last.
    run_step(7, "Frontend test", "frontend-test", run_frontend_test)?;
    run_step(8, "Test (xtask only)", "test", run_test)?;
    run_step(9, "Coverage", "coverage", run_coverage)?;
    run_step(10, "Audit", "audit", run_audit)?;

    println!("Validate OK ({})", elapsed_str(overall_start));
    Ok(())
}

/// Run a single step, printing the `[N/T]` result line.
///
/// `cmd` is the standalone xtask subcommand for this step;
/// on failure it is printed as an iterate-with hint so the
/// user re-runs the single failing gate (seconds) instead
/// of the whole pipeline (minutes).
fn run_step<F>(step: usize, name: &str, cmd: &str, f: F) -> Result<(), String>
where
    F: FnOnce() -> Result<String, String>,
{
    let start = Instant::now();
    match f() {
        Ok(detail) => {
            let time = elapsed_str(start);
            let full = if detail.is_empty() {
                time
            } else {
                format!("{detail}, {time}")
            };
            step_output(step, TOTAL_STEPS, name, "OK", &full);
            Ok(())
        }
        Err(e) => {
            step_output(step, TOTAL_STEPS, name, "FAILED", "");
            eprintln!("  -> iterate with: cargo xtask {cmd}");
            Err(e)
        }
    }
}

/// Fmt step -- returns empty detail on success. Auto-fixes
/// unless `check` selects the read-only path.
fn run_fmt(check: bool) -> Result<String, String> {
    if check {
        fmt_cmd::fmt_check()?;
    } else {
        fmt_cmd::fmt()?;
    }
    Ok(String::new())
}

/// Clippy step -- returns empty detail on success.
fn run_clippy() -> Result<String, String> {
    let r = clippy_cmd::clippy_check()?;
    match r.error {
        None => Ok(String::new()),
        Some(err) => {
            for line in r.items.iter().take(5) {
                eprintln!("  {line}");
            }
            Err(err)
        }
    }
}

/// Test step -- runs xtask's own tests only.
///
/// The coverage step runs `--workspace --exclude xtask`
/// under llvm-cov instrumentation, which executes every
/// non-xtask test. Running the full workspace tests
/// here too would duplicate that work. Restricting to
/// `-p xtask` keeps validate a full quality gate
/// without paying the duplication cost.
fn run_test() -> Result<String, String> {
    test_cmd::test_check_xtask()?;
    Ok(String::new())
}

/// Security-advisory step -- fails on any vulnerability
/// (RUSTSEC or npm). Advisory warnings are informational,
/// and a connectivity / missing-tool failure degrades to a
/// printed warning rather than failing the gate, so an
/// offline machine or fresh CI run is not blocked by a
/// transient outage.
fn run_audit() -> Result<String, String> {
    let r = audit::audit_check();
    if let Some(err) = r.error {
        return Err(err);
    }
    for w in &r.warnings {
        eprintln!("  warning: {w}");
    }
    Ok(r.detail)
}

/// Coverage step -- returns "N.N% >= 90%" detail.
fn run_coverage() -> Result<String, String> {
    let r = coverage::coverage_check()?;
    match r.error {
        None => Ok(format!(
            "{:.1}% >= {}%",
            r.line_pct,
            coverage::OVERALL_THRESHOLD,
        )),
        Some(failure) => Err(coverage::format_failure(&failure)),
    }
}

/// Duplication step -- returns detail string.
fn run_duplication() -> Result<String, String> {
    let r = dupes::dupes_check()?;
    if let Some(err) = r.error {
        Err(err)
    } else {
        Ok(r.detail)
    }
}

/// Frontend Prettier step -- auto-fixes unless `check`
/// selects the read-only path. Same skip/error semantics as
/// the frontend check step.
fn run_frontend_fmt(check: bool) -> Result<String, String> {
    format_frontend(frontend_fmt::frontend_fmt(check)?)
}

/// Frontend type check -- skips (pass) when there is no
/// frontend at all, but errors when a frontend exists with
/// its `node_modules` not installed (a silent skip there
/// would read as a pass).
fn run_frontend_check() -> Result<String, String> {
    format_frontend(frontend_check::frontend_check()?)
}

/// Frontend duplication step (jscpd).
fn run_frontend_dupes() -> Result<String, String> {
    format_frontend(frontend_dupes::frontend_dupes()?)
}

/// Frontend unit-test step (vitest).
fn run_frontend_test() -> Result<String, String> {
    format_frontend(frontend_test::frontend_test()?)
}

/// Shared mapping from a [`crate::frontend::FrontendResult`]
/// to a validate step detail string / error.
fn format_frontend(
    r: crate::frontend::FrontendResult,
) -> Result<String, String> {
    match r.error {
        None if r.skipped => Ok(format!("skipped: {}", r.detail)),
        None => Ok(r.detail),
        Some(err) => Err(err),
    }
}
