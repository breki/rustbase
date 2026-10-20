//! `validate` as data. Each gate is a [`Step`]; [`standard`] lists
//! the library's gates in their order, and [`run_all`] runs a list,
//! printing one `[N/M]` line per step. A project adds its own steps
//! to the list -- [`position`] finds where -- before running it:
//!
//! ```no_run
//! # use rustbase_xtask::{Config, step::{self, Step}};
//! # let cfg = Config::for_project("p", ".".into());
//! let mut steps = step::standard(&cfg, false);
//! let at = step::position(&steps, "Deny").map_or(steps.len(), |i| i + 1);
//! steps.insert(at, Step::new("Lint", "lint", || Ok(String::new())));
//! step::run_all(steps)?;
//! # Ok::<(), String>(())
//! ```

use std::time::Instant;

use crate::{
    Config, Strictness, audit, canon, cargo_test, clippy, coverage, deny,
    dep_age, doc, dupes, fmt, records,
};

/// Width of the step name column, dots included.
const STEP_NAME_WIDTH: usize = 14;

/// Diff lines the Fmt step shows.
const FMT_DIFF_LINES: usize = 20;

/// Diagnostics the Clippy and Doc steps show.
const DIAGNOSTIC_LINES: usize = 5;

/// One gate: the name on its `[N/M]` line, the `cargo xtask`
/// subcommand that re-runs it alone, and the work.
pub struct Step {
    name: &'static str,
    cmd: &'static str,
    run: Box<dyn FnOnce() -> Result<String, String>>,
}

impl Step {
    /// A step named `name`, re-run with `cargo xtask <cmd>` when it
    /// fails. `run` returns the detail printed after `OK` (empty for
    /// none) or the error.
    pub fn new(
        name: &'static str,
        cmd: &'static str,
        run: impl FnOnce() -> Result<String, String> + 'static,
    ) -> Self {
        Self {
            name,
            cmd,
            run: Box::new(run),
        }
    }

    /// The name printed on the step's `[N/M]` line.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The subcommand to re-run when this step fails.
    pub fn cmd(&self) -> &'static str {
        self.cmd
    }

    /// Run the step, consuming it.
    ///
    /// # Errors
    ///
    /// The gate's failure.
    pub fn run(self) -> Result<String, String> {
        (self.run)()
    }
}

/// The library's gates in the order `validate` runs them, each
/// re-run by the subcommand of the same name in the project's CLI.
/// `check` makes Fmt read-only (`fmt --check`) instead of fixing.
///
/// Dep-age leads: it is free on unchanged lockfiles and fails a
/// too-new dependency before anything compiles it. The cheap static
/// gates follow, Fmt first because it rewrites what later ones
/// read; then Test and Coverage; then Audit, which needs the network.
/// A project's static gates belong after Deny.
pub fn standard(cfg: &Config, check: bool) -> Vec<Step> {
    let c = cfg.clone();
    let strict = dep_age_strict(cfg, |k| std::env::var(k).ok());
    let mut steps = vec![Step::new("Dep-age", "dep-age-check", move || {
        dep_age_verdict(dep_age::check_changed_deps(&c, "HEAD"), strict)
    })];
    let c = cfg.clone();
    steps.push(Step::new("Fmt", "fmt", move || {
        let r = if check { fmt::check(&c) } else { fmt::fix(&c) };
        r.map_err(|f| f.report(FMT_DIFF_LINES))?;
        Ok(String::new())
    }));
    steps.push(gate(cfg, "Canon", "canon-check", canon::check));
    steps.push(gate(cfg, "Records", "records-check", records::check));
    steps.push(gate(cfg, "Duplication", "dupes", dupes::check));
    steps.push(gate(cfg, "Clippy", "clippy", |c| {
        clippy::check(c).map_err(|f| f.report(DIAGNOSTIC_LINES))?;
        Ok(String::new())
    }));
    steps.push(gate(cfg, "Doc", "doc", |c| {
        doc::check(c).map_err(|f| f.report(DIAGNOSTIC_LINES))?;
        Ok(String::new())
    }));
    steps.push(gate(cfg, "Deny", "deny", deny::check));
    steps.push(gate(cfg, "Test (xtask only)", "test", |c| {
        cargo_test::tooling_only(c).map_err(|f| f.to_string())?;
        Ok(String::new())
    }));
    steps.push(gate(cfg, "Coverage", "coverage", |c| {
        Ok(coverage::check(c)?.to_string())
    }));
    steps.push(gate(cfg, "Audit", "audit", |c| {
        audit::check(c, c.audit_npm_dir.as_deref(), Strictness::Warn)
    }));
    steps
}

/// A step running `f` on its own copy of `cfg`.
fn gate(
    cfg: &Config,
    name: &'static str,
    cmd: &'static str,
    f: fn(&Config) -> Result<String, String>,
) -> Step {
    let c = cfg.clone();
    Step::new(name, cmd, move || f(&c))
}

/// Index of the step named `name`.
pub fn position(steps: &[Step], name: &str) -> Option<usize> {
    steps.iter().position(|s| s.name == name)
}

/// Run `steps` in order, printing a `[N/M]` line for each and then
/// `Validate OK`, stopping at the first failure. A failed step
/// prints the subcommand that re-runs it alone.
///
/// # Errors
///
/// The first failing step's error.
pub fn run_all(steps: Vec<Step>) -> Result<(), String> {
    let start = Instant::now();
    let total = steps.len();
    for (i, step) in steps.into_iter().enumerate() {
        let (name, cmd) = (step.name, step.cmd);
        let step_start = Instant::now();
        match step.run() {
            Ok(detail) => {
                let time = elapsed(step_start);
                let full = if detail.is_empty() {
                    time
                } else {
                    format!("{detail}, {time}")
                };
                println!("{}", format_step(i + 1, total, name, "OK", &full));
            }
            Err(e) => {
                println!("{}", format_step(i + 1, total, name, "FAILED", ""));
                eprintln!("  -> iterate with: cargo xtask {cmd}");
                return Err(e);
            }
        }
    }
    println!("Validate OK ({})", elapsed(start));
    Ok(())
}

/// `[1/5] Fmt........... OK (0.3s)`
fn format_step(
    n: usize,
    total: usize,
    name: &str,
    status: &str,
    detail: &str,
) -> String {
    let dots = ".".repeat(STEP_NAME_WIDTH.saturating_sub(name.len()));
    if detail.is_empty() {
        format!("[{n}/{total}] {name}{dots} {status}")
    } else {
        format!("[{n}/{total}] {name}{dots} {status} ({detail})")
    }
}

/// Seconds since `start`, as `1.2s`.
fn elapsed(start: Instant) -> String {
    format!("{:.1}s", start.elapsed().as_secs_f64())
}

/// The Dep-age step's result: a fresh dependency always fails; a
/// check that could not finish fails only when `strict`, and warns
/// otherwise so an offline run is not blocked.
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

/// Strict when `cfg.dep_age_strict_env` is `1` or under CI
/// (`CI=true`, which GitHub Actions sets). `env` is injected so the
/// tests need no process environment.
fn dep_age_strict(cfg: &Config, env: impl Fn(&str) -> Option<String>) -> bool {
    env(&cfg.dep_age_strict_env).as_deref() == Some("1")
        || env("CI").as_deref() == Some("true")
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    fn cfg() -> Config {
        Config::for_project("p", "/w".into())
    }

    #[test]
    fn the_standard_gates_run_in_their_order() {
        let names: Vec<&str> =
            standard(&cfg(), true).iter().map(Step::name).collect();
        assert_eq!(
            names,
            [
                "Dep-age",
                "Fmt",
                "Canon",
                "Records",
                "Duplication",
                "Clippy",
                "Doc",
                "Deny",
                "Test (xtask only)",
                "Coverage",
                "Audit",
            ]
        );
    }

    #[test]
    fn every_standard_step_runs_against_its_config() {
        // In an empty directory the tools fail fast (no Cargo.toml
        // above a temp dir), Fmt only checks, and nothing is written.
        // The file-reading gates show the steps reach the config.
        let dir = crate::test_repo::empty_dir();
        let cfg = Config::for_project("p", dir.0.clone());
        for step in standard(&cfg, true) {
            let (name, cmd) = (step.name(), step.cmd());
            assert!(!cmd.is_empty(), "{name}");
            let got = step.run();
            match name {
                "Records" => {
                    let d = got.unwrap();
                    assert!(
                        d.starts_with("0 files, 0 entries; absent:"),
                        "{d}"
                    );
                }
                "Dep-age" | "Canon" | "Duplication" | "Fmt" | "Clippy"
                | "Doc" | "Deny" | "Test (xtask only)" | "Coverage"
                | "Audit" => {}
                other => panic!("untested step {other}"),
            }
        }
    }

    #[test]
    fn position_finds_a_step_by_name() {
        let steps = standard(&cfg(), true);
        assert_eq!(position(&steps, "Canon"), Some(2));
        assert_eq!(position(&steps, "Nope"), None);
    }

    #[test]
    fn run_all_stops_at_the_first_failure() {
        let ran = Rc::new(Cell::new(0));
        let step = |ok: bool| {
            let ran = Rc::clone(&ran);
            Step::new("S", "s", move || {
                ran.set(ran.get() + 1);
                if ok {
                    Ok(String::new())
                } else {
                    Err("boom".into())
                }
            })
        };
        assert_eq!(run_all(vec![step(true), step(true)]), Ok(()));
        assert_eq!(ran.get(), 2);
        assert_eq!(
            run_all(vec![step(true), step(false), step(true)]),
            Err("boom".into())
        );
        assert_eq!(ran.get(), 4, "the step after the failure ran");
    }

    #[test]
    fn format_step_pads_the_name_and_adds_the_detail() {
        assert_eq!(
            format_step(1, 5, "Fmt", "OK", "0.3s"),
            "[1/5] Fmt........... OK (0.3s)"
        );
        assert_eq!(
            format_step(2, 5, "Clippy", "FAILED", ""),
            "[2/5] Clippy........ FAILED"
        );
        assert_eq!(
            format_step(1, 1, "VeryLongStepName", "OK", ""),
            "[1/1] VeryLongStepName OK"
        );
        assert!(elapsed(Instant::now()).ends_with('s'));
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
    fn dep_age_strictness_comes_from_ci_or_the_project_variable() {
        let env = |pairs: &'static [(&str, &str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| (*v).into())
            }
        };
        let c = cfg();
        assert_eq!(c.dep_age_strict_env, "P_DEP_AGE_STRICT");
        assert!(!dep_age_strict(&c, env(&[])));
        assert!(dep_age_strict(&c, env(&[("CI", "true")])));
        assert!(dep_age_strict(&c, env(&[("P_DEP_AGE_STRICT", "1")])));
        assert!(!dep_age_strict(&c, env(&[("P_DEP_AGE_STRICT", "0")])));
        assert!(!dep_age_strict(&c, env(&[("CI", "false")])));
    }
}
