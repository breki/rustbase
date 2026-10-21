//! The four frontend gates, behind the `frontend` feature: Prettier,
//! svelte-check, jscpd and Vitest, each one npm script run in
//! `Config::frontend_dir`. The JS tools are the project's own npm dev
//! dependencies; the gates only map a script's exit status.
//!
//! A gate passes as skipped when there is no frontend, or when no
//! `frontend_dir` is set (reported apart, so a forgotten setting
//! shows), but fails -- never skips -- when the frontend exists with
//! its `node_modules` not installed, so a missing install cannot read
//! as a pass.

use std::path::Path;

use crate::process::{self, Captured};
use crate::step::{Step, names};
use crate::{Config, Failure, STEP_LINES};

/// The step detail of a gate that found no frontend.
pub const SKIPPED: &str = "skipped: no frontend";

/// The step detail of a gate given no `Config::frontend_dir`, kept
/// apart from [`SKIPPED`] so a project that forgot to set it can see so.
pub const NOT_CONFIGURED: &str = "skipped: frontend_dir not set";

/// One of the four gates; Prettier comes in two modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Gate {
    /// Prettier, fixing in place: `format`.
    Fmt,
    /// Prettier, read-only: `format:check`.
    FmtCheck,
    /// svelte-check: `check`.
    Check,
    /// jscpd, which enforces its own threshold: `dupes`.
    Dupes,
    /// Vitest: `test`.
    Test,
}

impl Gate {
    /// The four `validate` runs, in order: the static three, then the
    /// tests. `check` picks Prettier's read-only mode.
    #[must_use]
    pub fn all(check: bool) -> [Self; 4] {
        let fmt = if check { Self::FmtCheck } else { Self::Fmt };
        [fmt, Self::Check, Self::Dupes, Self::Test]
    }

    /// The step name, as `validate` prints it and `--skip` matches it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Fmt | Self::FmtCheck => names::FRONTEND_FMT,
            Self::Check => names::FRONTEND_CHECK,
            Self::Dupes => names::FRONTEND_DUPES,
            Self::Test => names::FRONTEND_TEST,
        }
    }

    /// The subcommand that re-runs it.
    #[must_use]
    pub fn subcommand(self) -> &'static str {
        match self {
            Self::Fmt | Self::FmtCheck => "frontend-fmt",
            Self::Check => "frontend-check",
            Self::Dupes => "frontend-dupes",
            Self::Test => "frontend-test",
        }
    }

    /// The step detail on a pass.
    #[must_use]
    pub fn ok_detail(self) -> &'static str {
        match self {
            Self::Dupes => "<= threshold",
            _ => "",
        }
    }

    fn script(self) -> &'static str {
        match self {
            Self::Fmt => "format",
            Self::FmtCheck => "format:check",
            Self::Check => "check",
            Self::Dupes => "dupes",
            Self::Test => "test",
        }
    }

    fn failure(self) -> &'static str {
        match self {
            Self::Fmt => "prettier failed",
            Self::FmtCheck => "prettier found unformatted files",
            Self::Check => "svelte-check reported errors",
            Self::Dupes => "frontend duplication exceeds threshold",
            Self::Test => "frontend unit tests failed",
        }
    }
}

/// What a gate did when it did not fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// There is no frontend; nothing ran.
    Skipped,
    /// No `Config::frontend_dir` is set; nothing ran.
    NotConfigured,
    /// The script passed.
    Passed,
}

/// What to do, given what is on disk.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Skip,
    NotInstalled,
    Run,
}

/// Decide from the two on-disk facts. Pure, so the skip-vs-error
/// boundary is tested without npm.
fn classify(has_package_json: bool, has_node_modules: bool) -> Action {
    match (has_package_json, has_node_modules) {
        (false, _) => Action::Skip,
        (true, false) => Action::NotInstalled,
        (true, true) => Action::Run,
    }
}

/// Run `gate` in the configured frontend, found from `cfg.root`.
///
/// # Errors
/// When the frontend exists but is not installed, npm cannot run,
/// or the script fails.
pub fn run(cfg: &Config, gate: Gate) -> Result<Outcome, Failure> {
    let Some(dir) = cfg.frontend_dir.as_ref().map(|d| cfg.root.join(d)) else {
        return Ok(Outcome::NotConfigured);
    };
    match classify(
        dir.join("package.json").is_file(),
        dir.join("node_modules").is_dir(),
    ) {
        Action::Skip => Ok(Outcome::Skipped),
        Action::NotInstalled => Err(not_installed(&dir).into()),
        Action::Run => {
            outcome(gate, &process::npm(&dir, &["run", gate.script()])?)
        }
    }
}

fn not_installed(dir: &Path) -> String {
    format!(
        "{} has a package.json but no node_modules -- run `npm install` \
         inside it",
        dir.display()
    )
}

/// Map a finished script to the gate's result, reported as every gate
/// reports a failed run: the end of the tool's stderr, else its stdout.
fn outcome(gate: Gate, c: &Captured) -> Result<Outcome, Failure> {
    if c.ok {
        return Ok(Outcome::Passed);
    }
    let summary = format!("{} ({})", gate.failure(), c.status);
    Err(Failure::of_run(summary, Vec::<String>::new(), c))
}

/// The four `validate` steps, in [`Gate::all`] order. A project puts
/// them after Deny: `step::insert_after(&mut steps, names::DENY, ..)`.
#[must_use]
pub fn steps(cfg: &Config, check: bool) -> Vec<Step> {
    Gate::all(check)
        .into_iter()
        .map(|gate| {
            let c = cfg.clone();
            Step::new(gate.name(), gate.subcommand(), move || {
                match run(&c, gate).map_err(|f| f.report(STEP_LINES))? {
                    Outcome::Skipped => Ok(SKIPPED.to_owned()),
                    Outcome::NotConfigured => Ok(NOT_CONFIGURED.to_owned()),
                    Outcome::Passed => Ok(gate.ok_detail().to_owned()),
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    fn failed(stdout: &str, stderr: &str) -> Captured {
        Captured {
            ok: false,
            status: "exit status: 1".into(),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    #[test]
    fn the_on_disk_facts_decide_skip_error_or_run() {
        assert_eq!(classify(false, false), Action::Skip);
        assert_eq!(classify(false, true), Action::Skip);
        assert_eq!(classify(true, false), Action::NotInstalled);
        assert_eq!(classify(true, true), Action::Run);
    }

    #[test]
    fn a_passing_script_passes() {
        let mut ok = failed("", "");
        ok.ok = true;
        assert_eq!(outcome(Gate::Test, &ok), Ok(Outcome::Passed));
        assert_eq!(Gate::Dupes.ok_detail(), "<= threshold");
        assert_eq!(Gate::Test.ok_detail(), "");
    }

    #[test]
    fn a_failure_reports_the_end_of_stderr() {
        // Real jscpd output: the table on stdout, the verdict on stderr.
        let stdout = "\n> rustbase-frontend@0.1.0 dupes\n> jscpd src\n\n\
            Found 1 clones.\n";
        let stderr = "Using config from .jscpd.json\n\
            ERROR: jscpd found too many duplicates (12.0%) over threshold \
            (6.0%)\n";
        let f = outcome(Gate::Dupes, &failed(stdout, stderr)).unwrap_err();
        assert_eq!(
            f.summary,
            "frontend duplication exceeds threshold (exit status: 1)"
        );
        assert_eq!(f.items.last().unwrap(), stderr.lines().nth(1).unwrap());
    }

    #[test]
    fn a_failure_with_nothing_on_stderr_reports_the_end_of_stdout() {
        // Real svelte-check output (human format, colour off).
        let stdout = "\n> rustbase-frontend@0.1.0 check\n\
            /w/frontend/src/zz_type.ts:1:7\n\
            Error: Type 'string' is not assignable to type 'number'. \n\
            svelte-check found 1 error and 0 warnings in 1 file\n";
        let f = outcome(Gate::Check, &failed(stdout, "")).unwrap_err();
        assert!(f.items.iter().any(|i| i.ends_with("zz_type.ts:1:7")), "{f}");
        assert!(f.items.iter().any(|i| i.starts_with("Error: Type")), "{f}");
    }

    #[test]
    fn a_silent_failure_still_says_something() {
        let f = outcome(Gate::Check, &failed("", "")).unwrap_err();
        assert_eq!(f.items, ["exit status: 1, and no output"]);
    }

    #[test]
    fn npms_missing_script_error_is_what_gets_reported() {
        let stderr = "npm error Missing script: \"dupes\"\n";
        let f = outcome(Gate::Dupes, &failed("", stderr)).unwrap_err();
        assert_eq!(f.items, ["npm error Missing script: \"dupes\""]);
    }

    #[test]
    fn check_picks_prettiers_read_only_mode() {
        assert_eq!(Gate::all(true)[0], Gate::FmtCheck);
        assert_eq!(Gate::all(false)[0], Gate::Fmt);
        assert_eq!(Gate::FmtCheck.script(), "format:check");
        assert_eq!(Gate::Fmt.script(), "format");
        assert_eq!(Gate::Fmt.name(), Gate::FmtCheck.name());
        assert_eq!(Gate::Fmt.failure(), "prettier failed");
    }

    #[test]
    fn no_configured_frontend_skips() {
        let cfg = Config::for_project("t", PathBuf::from("/nonexistent"));
        for gate in Gate::all(true) {
            assert_eq!(run(&cfg, gate), Ok(Outcome::NotConfigured));
        }
        let steps = steps(&cfg, true);
        assert_eq!(
            steps.into_iter().next().unwrap().run(),
            Ok(NOT_CONFIGURED.to_owned())
        );
    }

    #[test]
    fn the_frontend_is_found_under_root_not_the_working_dir() {
        // `frontend/` holds a package.json but no node_modules, so
        // finding it fails; the working directory has neither.
        let dir = crate::test_repo::empty_dir();
        fs::create_dir(dir.0.join("frontend")).unwrap();
        fs::write(dir.0.join("frontend/package.json"), "{}").unwrap();
        let mut cfg = Config::for_project("t", dir.0.clone());
        cfg.frontend_dir = Some("frontend".into());
        let f = run(&cfg, Gate::Check).unwrap_err();
        assert!(f.summary.contains("no node_modules"), "{f}");
        cfg.frontend_dir = Some("elsewhere".into());
        assert_eq!(run(&cfg, Gate::Check), Ok(Outcome::Skipped));
    }

    #[test]
    fn the_steps_are_named_after_the_gates_in_order() {
        let cfg = Config::for_project("t", PathBuf::from("/nonexistent"));
        let steps = steps(&cfg, true);
        let got: Vec<&str> = steps.iter().map(Step::name).collect();
        assert_eq!(
            got,
            [
                "Frontend fmt",
                "Frontend check",
                "Frontend dupes",
                "Frontend test"
            ]
        );
        for s in steps {
            assert_eq!(s.run(), Ok(NOT_CONFIGURED.to_owned()));
        }
    }
}
