//! The four frontend gates, behind the `frontend` feature: Prettier,
//! svelte-check, jscpd and Vitest, each one npm script run in
//! `Config::frontend_dir`. The JS tools are the project's own npm dev
//! dependencies; the gates only map a script's exit status.
//!
//! A gate passes as skipped when there is no frontend, but fails --
//! never skips -- when the frontend exists with its `node_modules`
//! not installed, so a missing install cannot read as a pass.

use std::path::Path;

use crate::Config;
use crate::process::{self, Captured};
use crate::step::{Step, names};

/// Lines of a failing script's output kept in the error.
const MAX_OUTPUT_LINES: usize = 10;

/// One of the four gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Prettier: `format`, or `format:check` when read-only.
    Fmt,
    /// svelte-check: `check`.
    Check,
    /// jscpd, which enforces its own threshold: `dupes`.
    Dupes,
    /// Vitest: `test`.
    Test,
}

impl Gate {
    /// All four, in `validate` order: the static three, then the
    /// tests.
    pub const ALL: [Self; 4] =
        [Self::Fmt, Self::Check, Self::Dupes, Self::Test];

    /// The step name, as `validate` prints it and `--skip` matches it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Fmt => names::FRONTEND_FMT,
            Self::Check => names::FRONTEND_CHECK,
            Self::Dupes => names::FRONTEND_DUPES,
            Self::Test => names::FRONTEND_TEST,
        }
    }

    /// The subcommand that re-runs it.
    #[must_use]
    pub fn subcommand(self) -> &'static str {
        match self {
            Self::Fmt => "frontend-fmt",
            Self::Check => "frontend-check",
            Self::Dupes => "frontend-dupes",
            Self::Test => "frontend-test",
        }
    }

    /// The npm script; `check` makes Fmt read-only.
    fn script(self, check: bool) -> &'static str {
        match self {
            Self::Fmt if check => "format:check",
            Self::Fmt => "format",
            Self::Check => "check",
            Self::Dupes => "dupes",
            Self::Test => "test",
        }
    }

    /// The step detail on a pass.
    fn ok_detail(self) -> &'static str {
        match self {
            Self::Dupes => "<= threshold",
            _ => "",
        }
    }

    /// The error on a failing script.
    fn failure(self, check: bool) -> &'static str {
        match self {
            Self::Fmt if check => "prettier found unformatted files",
            Self::Fmt => "prettier failed",
            Self::Check => "svelte-check reported errors",
            Self::Dupes => "frontend duplication exceeds threshold",
            Self::Test => "frontend unit tests failed",
        }
    }
}

/// What a gate did when it did not fail.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// There is no frontend; nothing ran.
    Skipped,
    /// The script passed; the step detail.
    Passed(String),
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

/// Run `gate` in the configured frontend. `check` makes Fmt
/// read-only (`format:check`) instead of fixing in place.
///
/// # Errors
/// When the frontend exists but is not installed, npm cannot run,
/// or the script fails (with the start of its output).
pub fn run(cfg: &Config, gate: Gate, check: bool) -> Result<Outcome, String> {
    let Some(dir) = cfg.frontend_dir.as_ref().map(|d| cfg.root.join(d)) else {
        return Ok(Outcome::Skipped);
    };
    match classify(
        dir.join("package.json").is_file(),
        dir.join("node_modules").is_dir(),
    ) {
        Action::Skip => Ok(Outcome::Skipped),
        Action::NotInstalled => Err(not_installed(&dir)),
        Action::Run => outcome(
            gate,
            check,
            &process::npm(&dir, &["run", gate.script(check)])?,
        ),
    }
}

fn not_installed(dir: &Path) -> String {
    format!(
        "{} has a package.json but no node_modules -- run `npm install` \
         inside it",
        dir.display()
    )
}

/// Map a finished script to the gate's result.
fn outcome(gate: Gate, check: bool, c: &Captured) -> Result<Outcome, String> {
    if c.ok {
        return Ok(Outcome::Passed(gate.ok_detail().to_owned()));
    }
    let output = format!("{}{}", c.stdout, c.stderr);
    let head: Vec<&str> = output.lines().take(MAX_OUTPUT_LINES).collect();
    let mut err = format!("{} ({})", gate.failure(check), c.status);
    for line in head {
        err.push_str("\n  ");
        err.push_str(line);
    }
    Err(err)
}

/// The four `validate` steps, in [`Gate::ALL`] order. A project puts
/// them after Deny: `step::insert_after(&mut steps, names::DENY, ..)`.
#[must_use]
pub fn steps(cfg: &Config, check: bool) -> Vec<Step> {
    Gate::ALL
        .into_iter()
        .map(|gate| {
            let c = cfg.clone();
            Step::new(gate.name(), gate.subcommand(), move || {
                Ok(match run(&c, gate, check)? {
                    Outcome::Skipped => "skipped: no frontend".to_owned(),
                    Outcome::Passed(detail) => detail,
                })
            })
        })
        .collect()
}

/// The standalone `cargo xtask frontend-<gate>` line.
///
/// # Errors
/// As [`run`].
pub fn command(cfg: &Config, gate: Gate, check: bool) -> Result<(), String> {
    match run(cfg, gate, check)? {
        Outcome::Skipped => println!("{} skipped: no frontend", gate.name()),
        Outcome::Passed(_) => println!("{} OK", gate.name()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    fn captured(ok: bool, stdout: &str) -> Captured {
        Captured {
            ok,
            status: if ok {
                "exit status: 0"
            } else {
                "exit status: 1"
            }
            .into(),
            stdout: stdout.into(),
            stderr: String::new(),
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
    fn a_passing_script_gives_the_gates_detail() {
        let ok = captured(true, "");
        assert_eq!(
            outcome(Gate::Dupes, false, &ok),
            Ok(Outcome::Passed("<= threshold".into()))
        );
        assert_eq!(
            outcome(Gate::Test, false, &ok),
            Ok(Outcome::Passed(String::new()))
        );
    }

    #[test]
    fn a_failing_script_keeps_the_start_of_its_output() {
        let long = (0..20).fold(String::new(), |mut s, i| {
            let _ = writeln!(s, "line {i}");
            s
        });
        let err =
            outcome(Gate::Fmt, true, &captured(false, &long)).unwrap_err();
        assert!(err.starts_with("prettier found unformatted files"), "{err}");
        assert!(err.contains("exit status: 1"), "{err}");
        assert!(err.contains("line 9") && !err.contains("line 10"), "{err}");
    }

    #[test]
    fn fmt_runs_the_read_only_script_under_check() {
        assert_eq!(Gate::Fmt.script(true), "format:check");
        assert_eq!(Gate::Fmt.script(false), "format");
        assert_eq!(Gate::Fmt.failure(false), "prettier failed");
        assert_eq!(Gate::Check.script(true), "check");
    }

    #[test]
    fn no_configured_frontend_skips() {
        let cfg = Config::for_project("t", PathBuf::from("/nonexistent"));
        for gate in Gate::ALL {
            assert_eq!(run(&cfg, gate, true), Ok(Outcome::Skipped));
        }
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
        let err = run(&cfg, Gate::Check, true).unwrap_err();
        assert!(err.contains("no node_modules"), "{err}");
        cfg.frontend_dir = Some("elsewhere".into());
        assert_eq!(run(&cfg, Gate::Check, true), Ok(Outcome::Skipped));
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
            assert_eq!(s.run(), Ok("skipped: no frontend".to_owned()));
        }
        for gate in Gate::ALL {
            assert!(command(&cfg, gate, false).is_ok());
            assert!(gate.subcommand().starts_with("frontend-"));
        }
    }
}
