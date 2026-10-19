use std::time::Instant;

use crate::audit;
use crate::canon;
use crate::clippy_cmd;
use crate::coverage;
use crate::deny;
use crate::doc_cmd;
use crate::dupes;
use crate::fmt_cmd;
use crate::frontend_check;
use crate::frontend_dupes;
use crate::frontend_fmt;
use crate::frontend_test;
use crate::helpers::{elapsed_str, step_output};
use crate::test_cmd;
use rustbase_xtask::dep_age;

/// A single validation step: a display name, the standalone
/// xtask subcommand that runs it alone (the `iterate with`
/// hint), and the closure that performs it.
struct Step {
    name: &'static str,
    cmd: &'static str,
    run: Box<dyn FnOnce() -> Result<String, String>>,
}

/// Build the ordered list of validation steps.
///
/// Both the per-step index and the total derive from this one
/// list, so inserting a gate is a single line here -- there is
/// no hand-maintained `TOTAL_STEPS` counter and no per-call
/// index to renumber (and silently get wrong, printing
/// `[5/9]` twice). A unit test resolves every `cmd` through the
/// real CLI, so a renamed subcommand cannot leave the
/// `iterate with` hint pointing at a command that no longer
/// exists.
///
/// Ordering rationale: Dep-age first, then cheap static gates,
/// then the expensive dynamic gates -- see [`validate`].
fn steps(check: bool) -> Vec<Step> {
    fn boxed(
        f: impl FnOnce() -> Result<String, String> + 'static,
    ) -> Box<dyn FnOnce() -> Result<String, String>> {
        Box::new(f)
    }
    vec![
        // Fail fast on a within-cooldown dependency before
        // compiling it.
        Step {
            name: "Dep-age",
            cmd: "dep-age-check",
            run: boxed(run_dep_age),
        },
        // Cheap static gates first ...
        Step {
            name: "Fmt",
            cmd: "fmt",
            run: boxed(move || run_fmt(check)),
        },
        // Reads markdown only, so it needs no compilation and
        // sits ahead of every gate that does.
        Step {
            name: "Canon",
            cmd: "canon-check",
            run: boxed(canon::canon_check_detail),
        },
        Step {
            name: "Duplication",
            cmd: "dupes",
            run: boxed(run_duplication),
        },
        Step {
            name: "Clippy",
            cmd: "clippy",
            run: boxed(run_clippy),
        },
        // Reuses the metadata Clippy just produced, so both rustdoc
        // passes cost about a second warm.
        Step {
            name: "Doc",
            cmd: "doc",
            run: boxed(run_doc),
        },
        // Offline supply-chain gate -- see Audit (network) at the
        // end. Placed among the static gates because it compiles
        // nothing and reaches no network.
        Step {
            name: "Deny",
            cmd: "deny",
            run: boxed(run_deny),
        },
        Step {
            name: "Frontend fmt",
            cmd: "frontend-fmt",
            run: boxed(move || run_frontend_fmt(check)),
        },
        Step {
            name: "Frontend check",
            cmd: "frontend-check",
            run: boxed(run_frontend_check),
        },
        Step {
            name: "Frontend dupes",
            cmd: "frontend-dupes",
            run: boxed(run_frontend_dupes),
        },
        // ... expensive dynamic gates last.
        Step {
            name: "Frontend test",
            cmd: "frontend-test",
            run: boxed(run_frontend_test),
        },
        Step {
            name: "Test (xtask only)",
            cmd: "test",
            run: boxed(run_test),
        },
        Step {
            name: "Coverage",
            cmd: "coverage",
            run: boxed(run_coverage),
        },
        Step {
            name: "Audit",
            cmd: "audit",
            run: boxed(run_audit),
        },
    ]
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
/// so it never blocks the local gates behind the network.
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

    let steps = steps(check);
    let total = steps.len();
    for (i, step) in steps.into_iter().enumerate() {
        run_step(i + 1, total, step.name, step.cmd, step.run)?;
    }

    println!("Validate OK ({})", elapsed_str(overall_start));
    Ok(())
}

/// Run a single step, printing the `[N/T]` result line.
///
/// `total` is the step count (`steps().len()`), so the `[N/T]`
/// denominator tracks the list automatically. `cmd` is the
/// standalone xtask subcommand for this step; on failure it is
/// printed as an iterate-with hint so the user re-runs the
/// single failing gate (seconds) instead of the whole pipeline
/// (minutes).
fn run_step<F>(
    step: usize,
    total: usize,
    name: &str,
    cmd: &str,
    f: F,
) -> Result<(), String>
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
            step_output(step, total, name, "OK", &full);
            Ok(())
        }
        Err(e) => {
            step_output(step, total, name, "FAILED", "");
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

/// Deny step -- offline licence/bans/sources gate. Unlike Audit
/// it needs no network, so it runs here (and on every push in CI)
/// rather than only at release. A missing `cargo-deny` is a hard
/// error, not a degrade-to-warning: with no network to be down, a
/// machine that cannot run it is simply missing a tool.
fn run_deny() -> Result<String, String> {
    deny::deny()?;
    Ok(String::new())
}

/// Doc step -- both rustdoc passes; see [`doc_cmd`]. Returns empty
/// detail on success.
fn run_doc() -> Result<String, String> {
    match doc_cmd::doc_check()? {
        None => Ok(String::new()),
        Some(failure) => {
            // Keep this at least `doc_cmd`'s STDERR_TAIL_LINES, or
            // the fallback's last line -- the cause -- is cut off.
            for line in failure.items.iter().take(5) {
                eprintln!("  {line}");
            }
            Err(failure.summary)
        }
    }
}

/// Dep-age step -- cooldown-checks only dependencies added or
/// bumped in the working tree versus HEAD. A within-cooldown
/// dependency fails the gate. A check that could not finish
/// (missing baseline, unreachable registry, unreadable
/// `Cargo.lock`) warns, so an offline run is not blocked --
/// unless [`dep_age_strict`] says to fail.
fn run_dep_age() -> Result<String, String> {
    let strict = dep_age_strict(|k| std::env::var(k).ok());
    let cfg = crate::helpers::tasks_config();
    dep_age_verdict(dep_age::check_changed_deps(&cfg, "HEAD"), strict)
}

/// The step's result: a fresh dependency always fails; a check
/// that could not finish fails only when `strict`.
fn dep_age_verdict(
    r: dep_age::DepAgeResult,
    strict: bool,
) -> Result<String, String> {
    if let Some(err) = r.error {
        return Err(err);
    }
    if strict && !r.warnings.is_empty() {
        return Err(format!(
            "dep-age could not complete: {}",
            r.warnings.join("; ")
        ));
    }
    for w in &r.warnings {
        eprintln!("  warning: {w}");
    }
    Ok(r.detail)
}

/// Strict when `RUSTBASE_DEP_AGE_STRICT=1` or under CI
/// (`CI=true`, which GitHub Actions sets). `env` is injected so
/// the tests need no process environment.
fn dep_age_strict(env: impl Fn(&str) -> Option<String>) -> bool {
    env("RUSTBASE_DEP_AGE_STRICT").as_deref() == Some("1")
        || env("CI").as_deref() == Some("true")
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
                cli.find_subcommand(step.cmd).is_some(),
                "validate step {:?} hints `cargo xtask {}`, \
                 which is not a real subcommand",
                step.name,
                step.cmd,
            );
        }
    }

    #[test]
    fn dep_age_warnings_fail_only_when_strict() {
        let incomplete = || dep_age::DepAgeResult {
            detail: "0 changed".into(),
            error: None,
            warnings: vec!["registry unreachable".into()],
        };
        assert!(dep_age_verdict(incomplete(), false).is_ok());
        let err = dep_age_verdict(incomplete(), true).unwrap_err();
        assert!(err.contains("registry unreachable"), "{err}");
    }

    #[test]
    fn dep_age_strictness_comes_from_ci_or_opt_in() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| (*v).into())
            }
        };
        assert!(!dep_age_strict(env(&[])));
        assert!(dep_age_strict(env(&[("CI", "true")])));
        assert!(dep_age_strict(env(&[("RUSTBASE_DEP_AGE_STRICT", "1")])));
        assert!(!dep_age_strict(env(&[("RUSTBASE_DEP_AGE_STRICT", "0")])));
        assert!(!dep_age_strict(env(&[("CI", "false")])));
    }

    #[test]
    fn steps_list_is_non_empty() {
        // The `[N/total]` denominator derives from this length;
        // an empty list would silently disable the whole gate.
        assert!(!steps(false).is_empty(), "validate has no steps");
    }
}
