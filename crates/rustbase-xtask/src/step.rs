//! `validate` as data. Each gate is a [`Step`]; [`standard`] lists
//! the library's gates in their order, and [`run_all`] runs a list,
//! printing one `[N/M]` line per step. A project adds its own steps
//! to the list -- [`insert_after`] places them by a step's name in
//! [`names`] -- before running it:
//!
//! ```no_run
//! # use rustbase_xtask::{Config, step::{self, Step}};
//! # let cfg = Config::for_project("p", ".".into());
//! let mut steps = step::standard(&cfg, false);
//! let lint = Step::new("Lint", "lint", || Ok(String::new()));
//! step::insert_after(&mut steps, step::names::DENY, [lint])?;
//! let tee = rustbase_xtask::tee::Tee::open(&cfg.root, "validate")?;
//! let result = step::run_all(steps, &tee);
//! if let Err(e) = &result {
//!     // The caller prints the error, so it logs it too.
//!     tee.record(&format!("error: {e}"));
//! }
//! result?;
//! # Ok::<(), String>(())
//! ```

use std::time::Instant;

use crate::{
    Config, FMT_DIFF_LINES, STEP_LINES, Strictness, audit, canon, cargo_test,
    clippy, coverage, deny, dep_age, doc, dupes, fmt, records, scripts,
    tee::Tee,
};

/// Width of the step name column, dots included.
const STEP_NAME_WIDTH: usize = 14;

/// The names of the standard steps, as their `[N/M]` lines print
/// them: what [`position`] and [`insert_after`] find a step by.
pub mod names {
    /// The cooldown check of changed dependencies.
    pub const DEP_AGE: &str = "Dep-age";
    /// Formatting.
    pub const FMT: &str = "Fmt";
    /// The claims canon prose makes.
    pub const CANON: &str = "Canon";
    /// The record files.
    pub const RECORDS: &str = "Records";
    /// Scripts staged executable.
    pub const SCRIPTS: &str = "Scripts";
    /// Code duplication.
    pub const DUPLICATION: &str = "Duplication";
    /// Lints.
    pub const CLIPPY: &str = "Clippy";
    /// Doc comments and links.
    pub const DOC: &str = "Doc";
    /// Licences, bans and sources; the last static gate.
    pub const DENY: &str = "Deny";
    /// The `xtask` package's tests; Coverage runs every other
    /// crate's. Its re-run, `test`, runs them all.
    pub const TEST: &str = "Test (xtask only)";
    /// Line coverage.
    pub const COVERAGE: &str = "Coverage";
    /// Security advisories.
    pub const AUDIT: &str = "Audit";
    /// Frontend formatting (Prettier).
    #[cfg(feature = "frontend")]
    pub const FRONTEND_FMT: &str = "Frontend fmt";
    /// Frontend type check (svelte-check).
    #[cfg(feature = "frontend")]
    pub const FRONTEND_CHECK: &str = "Frontend check";
    /// Frontend duplication (jscpd).
    #[cfg(feature = "frontend")]
    pub const FRONTEND_DUPES: &str = "Frontend dupes";
    /// Frontend unit tests (Vitest).
    #[cfg(feature = "frontend")]
    pub const FRONTEND_TEST: &str = "Frontend test";
}

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

/// The name and subcommand; the work is a closure, which has no
/// `Debug`.
impl std::fmt::Debug for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Step")
            .field("name", &self.name)
            .field("cmd", &self.cmd)
            .finish_non_exhaustive()
    }
}

/// The library's gates in the order `validate` runs them. Each
/// carries the subcommand that re-runs it (`Step::cmd`), whose name
/// can differ from the step's: Duplication is `dupes`, Dep-age
/// `dep-age-check`.
/// `check` makes Fmt read-only (`fmt --check`) instead of fixing.
///
/// Dep-age leads: it is free on unchanged lockfiles and fails a
/// too-new dependency before anything compiles it. The cheap static
/// gates follow, Fmt first because it rewrites what later ones
/// read; then Test and Coverage; then Audit, which needs the network.
/// A project's static gates belong after Deny.
pub fn standard(cfg: &Config, check: bool) -> Vec<Step> {
    let c = cfg.clone();
    let mut steps =
        vec![Step::new(names::DEP_AGE, "dep-age-check", move || {
            dep_age::verdict(dep_age::check_changed_deps(&c, "HEAD"))
        })];
    let c = cfg.clone();
    steps.push(Step::new(names::FMT, "fmt", move || {
        let r = if check { fmt::check(&c) } else { fmt::fix(&c) };
        r.map_err(|f| f.report(FMT_DIFF_LINES))?;
        Ok(String::new())
    }));
    steps.push(gate(cfg, names::CANON, "canon-check", canon::check));
    steps.push(gate(cfg, names::RECORDS, "records-check", records::check));
    steps.push(gate(cfg, names::SCRIPTS, "scripts-check", scripts::check));
    steps.push(gate(cfg, names::DUPLICATION, "dupes", dupes::check));
    steps.push(gate(cfg, names::CLIPPY, "clippy", |c| {
        clippy::check(c).map_err(|f| f.report(STEP_LINES))?;
        Ok(String::new())
    }));
    steps.push(gate(cfg, names::DOC, "doc", |c| {
        doc::check(c).map_err(|f| f.report(doc::STDERR_TAIL_LINES))?;
        Ok(String::new())
    }));
    steps.push(gate(cfg, names::DENY, "deny", deny::check));
    steps.push(gate(cfg, names::TEST, "test", |c| {
        cargo_test::tooling_only(c).map_err(|f| f.to_string())?;
        Ok(String::new())
    }));
    steps.push(gate(cfg, names::COVERAGE, "coverage", |c| {
        Ok(coverage::check(c)?.to_string())
    }));
    steps.push(gate(cfg, names::AUDIT, "audit", |c| {
        audit::check(c, c.npm_dir().as_deref(), Strictness::Warn)
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

/// Insert `new` right after the step named `anchor`, in order.
///
/// # Errors
///
/// When no step is named `anchor`.
pub fn insert_after(
    steps: &mut Vec<Step>,
    anchor: &str,
    new: impl IntoIterator<Item = Step>,
) -> Result<(), String> {
    let at = position(steps, anchor)
        .ok_or_else(|| format!("no validate step named {anchor:?}"))?
        + 1;
    steps.splice(at..at, new);
    Ok(())
}

/// Run `steps` in order, printing a `[N/M]` line for each and then
/// `Validate OK`, stopping at the first failure. A failed step
/// prints the subcommand that re-runs it alone. Every line also goes
/// to `tee`'s log, but the returned error does not: the caller prints
/// it, so the caller writes it to the log too (`tee.record`) in the
/// words it prints, as the module example does. Output a gate prints
/// itself, such as a warning, is not in the log.
///
/// # Errors
///
/// The first failing step's error.
pub fn run_all(steps: Vec<Step>, tee: &Tee) -> Result<(), String> {
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
                tee.println(&format_step(i + 1, total, name, "OK", &full));
            }
            Err(e) => {
                tee.println(&format_step(i + 1, total, name, "FAILED", ""));
                tee.eprintln(&format!("  -> iterate with: cargo xtask {cmd}"));
                return Err(e);
            }
        }
    }
    tee.println(&format!("Validate OK ({})", elapsed(start)));
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
                "Scripts",
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
        // With no workspace the cargo gates fail fast, Fmt only
        // checks, and nothing is written. Audit is left out: it can
        // reach the advisory database over the network.
        let dir = crate::test_repo::outside_any_workspace();
        let cfg = Config::for_project("p", dir.0.clone());
        for step in standard(&cfg, true) {
            let name = step.name();
            match name {
                names::AUDIT => {}
                names::DEP_AGE => {
                    let e = step.run().unwrap_err();
                    assert!(e.contains("Cargo.lock: cannot read"), "{e}");
                }
                names::RECORDS => {
                    let d = step.run().unwrap();
                    assert!(
                        d.starts_with("0 files, 0 entries; absent:"),
                        "{d}"
                    );
                }
                names::SCRIPTS => {
                    let d = step.run().unwrap();
                    assert_eq!(d, "skipped: not in a git work tree");
                }
                names::CANON => {
                    let d = step.run().unwrap();
                    assert!(d.starts_with("0 files"), "no canon to read: {d}");
                }
                names::FMT
                | names::DUPLICATION
                | names::CLIPPY
                | names::DOC
                | names::DENY
                | names::TEST
                | names::COVERAGE => {
                    assert!(
                        step.run().is_err(),
                        "{name} passed with no workspace"
                    );
                }
                other => panic!("untested step {other}"),
            }
        }
    }

    #[test]
    fn insert_after_places_steps_behind_their_anchor() {
        let mut steps = standard(&cfg(), true);
        let new = |n| Step::new(n, "x", || Ok(String::new()));
        insert_after(&mut steps, names::DENY, [new("A"), new("B")]).unwrap();
        let at = position(&steps, names::DENY).unwrap();
        assert_eq!(steps[at + 1].name(), "A");
        assert_eq!(steps[at + 2].name(), "B");
        assert_eq!(steps[at + 3].name(), names::TEST);
        let e = insert_after(&mut steps, "Nope", [new("C")]).unwrap_err();
        assert!(e.contains("\"Nope\""), "{e}");
        assert_eq!(
            format!("{:?}", new("A")),
            "Step { name: \"A\", cmd: \"x\", .. }"
        );
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
        let dir = crate::test_repo::empty_dir();
        let log = || {
            std::fs::read_to_string(dir.0.join("target/validate.log")).unwrap()
        };
        let tee = Tee::open(&dir.0, "validate").unwrap();
        assert_eq!(run_all(vec![step(true), step(true)], &tee), Ok(()));
        assert_eq!(ran.get(), 2);
        let ok = log();
        assert!(ok.starts_with("[1/2] S"), "{ok}");
        assert!(ok.contains("\nValidate OK ("), "{ok}");
        drop(tee);
        let tee = Tee::open(&dir.0, "validate").unwrap();
        assert_eq!(
            run_all(vec![step(true), step(false), step(true)], &tee),
            Err("boom".into())
        );
        assert_eq!(ran.get(), 4, "the step after the failure ran");
        // The error itself is left to the caller, which prints it.
        let failed = log();
        assert!(failed.contains("[2/3] S............. FAILED"), "{failed}");
        assert!(
            failed.ends_with("iterate with: cargo xtask s\n"),
            "{failed}"
        );
        assert!(!failed.contains("[3/3]"), "{failed}");
        assert!(run_all(vec![step(true)], &Tee::terminal()).is_ok());
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
}
