//! The four frontend gates, behind the `frontend` feature: Prettier,
//! svelte-check, jscpd and Vitest, each one npm script run in
//! `Config::frontend_dir`. The JS tools are the project's own npm dev
//! dependencies; the gates only map a script's exit status. Each run's
//! whole output goes to `target/<subcommand>.log` for a person to read;
//! the tools' formats differ by version and by environment, so nothing
//! parses it.
//!
//! A gate passes as skipped when there is no frontend, or when no
//! `frontend_dir` is set (reported apart, so a forgotten setting
//! shows), but fails -- never skips -- when the frontend exists with
//! its `node_modules` not installed, so a missing install cannot read
//! as a pass.

use std::path::{Path, PathBuf};

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
    let none: [&str; 0] = [];
    run_with_args(cfg, gate, &none)
}

/// [`run`], passing `extra` to the gate's script after `--`, unchanged:
/// `frontend-test`'s path filters, and any Vitest options given after
/// its own `--`.
///
/// # Errors
/// As [`run`].
pub(crate) fn run_with_args(
    cfg: &Config,
    gate: Gate,
    extra: &[impl AsRef<str>],
) -> Result<Outcome, Failure> {
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
            let args = npm_args(gate, extra);
            let c = process::npm(&dir, &args)?;
            let log = write_log(&cfg.root, gate, &args, &c);
            outcome(gate, &c, log.as_deref().map_err(String::as_str))
        }
    }
}

/// `npm`'s arguments for `gate`, with `extra` behind the `--` that
/// stops npm reading them as its own.
fn npm_args(gate: Gate, extra: &[impl AsRef<str>]) -> Vec<&str> {
    let mut args = vec!["run", gate.script()];
    if !extra.is_empty() {
        args.push("--");
        args.extend(extra.iter().map(AsRef::as_ref));
    }
    args
}

/// Write a finished run's whole output to `target/<subcommand>.log`
/// under `root`, replacing the last run's, and return the log's
/// absolute path, made so even when `root` is relative. The file is
/// for reading: nothing parses it.
///
/// Prettier's fix and `--check` modes both write `frontend-fmt.log`,
/// and the last run to finish replaces it. Its first two lines say
/// which run it was: the npm arguments, then the exit status.
fn write_log(
    root: &Path,
    gate: Gate,
    args: &[&str],
    c: &Captured,
) -> Result<PathBuf, String> {
    let path = root
        .join("target")
        .join(format!("{}.log", gate.subcommand()));
    let path = std::path::absolute(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    // Rust's string-literal escaping, not a shell's: it keeps each
    // argument whole whatever it holds.
    let text = format!(
        "$ npm {args:?}\n{}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        c.status, c.stdout, c.stderr
    );
    path.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, text))
        .map(|()| path.clone())
        .map_err(|e| format!("{}: {e}", path.display()))
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
/// The summary names the log holding the whole run, since
/// `Failure::report` always prints the summary and cuts only the items
/// (to `STEP_LINES` under `validate`).
fn outcome(
    gate: Gate,
    c: &Captured,
    log: Result<&Path, &str>,
) -> Result<Outcome, Failure> {
    if c.ok {
        return Ok(Outcome::Passed);
    }
    let saved = match log {
        Ok(path) => format!("full output in {}", path.display()),
        Err(e) => format!("output not saved: {e}"),
    };
    let summary = format!("{} ({}); {saved}", gate.failure(), c.status);
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

    use super::*;

    fn failed(stdout: &str, stderr: &str) -> Captured {
        Captured {
            ok: false,
            status: "exit status: 1".into(),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    fn passed(stdout: &str) -> Captured {
        Captured {
            ok: true,
            status: "exit status: 0".into(),
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    /// Where `write_log` puts `gate`'s log under a root of `/r`.
    fn log(gate: Gate) -> PathBuf {
        PathBuf::from("/r/target").join(format!("{}.log", gate.subcommand()))
    }

    #[test]
    fn a_failure_names_the_log_or_says_why_there_is_none() {
        let c = failed("", "boom\n");
        let path = log(Gate::Test);
        let f = outcome(Gate::Test, &c, Ok(&path)).unwrap_err();
        assert_eq!(
            f.summary,
            format!(
                "frontend unit tests failed (exit status: 1); \
                 full output in {}",
                path.display()
            )
        );
        assert_eq!(f.items, ["boom"]);
        let f = outcome(Gate::Test, &c, Err("denied")).unwrap_err();
        assert_eq!(
            f.summary,
            "frontend unit tests failed (exit status: 1); \
             output not saved: denied"
        );
    }

    #[test]
    fn the_log_holds_the_last_run_whole_at_an_absolute_path() {
        // Absolute, so the path in a failure resolves from any directory.
        let dir = crate::test_repo::empty_dir();
        let path = dir.0.join("target/frontend-test.log");
        let first = failed("RUN v4\n FAIL a.test.ts\n", "AssertionError\n");
        assert_eq!(
            write_log(&dir.0, Gate::Test, &["run", "test"], &first),
            Ok(path.clone())
        );
        let text = fs::read_to_string(&path).unwrap();
        for part in [
            r#"$ npm ["run", "test"]"#,
            "exit status: 1",
            "FAIL a.test.ts",
        ] {
            assert!(text.contains(part), "{part}: {text}");
        }
        assert!(text.contains("AssertionError"), "{text}");
        write_log(
            &dir.0,
            Gate::Test,
            &["run", "test"],
            &passed("all passed\n"),
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("exit status: 0"), "{text}");
        assert!(text.contains("all passed"), "{text}");
        for stale in ["exit status: 1", "FAIL"] {
            assert!(!text.contains(stale), "{stale}: {text}");
        }
    }

    #[test]
    fn a_log_that_cannot_be_written_says_why() {
        // `target` is a file, so the log's folder cannot be made.
        let dir = crate::test_repo::empty_dir();
        fs::write(dir.0.join("target"), "").unwrap();
        let err =
            write_log(&dir.0, Gate::Check, &["run", "check"], &failed("", ""))
                .unwrap_err();
        assert!(err.contains("target"), "{err}");
        // The folder exists but the write fails: the log is a folder.
        let dir = crate::test_repo::empty_dir();
        fs::create_dir_all(dir.0.join("target/frontend-check.log")).unwrap();
        let err =
            write_log(&dir.0, Gate::Check, &["run", "check"], &failed("", ""))
                .unwrap_err();
        assert!(err.contains("frontend-check.log"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_relative_root_still_names_an_absolute_log() {
        // `Config::root` may be relative; reach the temp dir from the
        // working directory by `..` steps.
        let dir = crate::test_repo::empty_dir();
        let cwd = std::env::current_dir().unwrap();
        let up = "../".repeat(cwd.components().count() - 1);
        let rel = PathBuf::from(up).join(dir.0.strip_prefix("/").unwrap());
        let path =
            write_log(&rel, Gate::Test, &["run", "test"], &passed("")).unwrap();
        assert!(path.is_absolute(), "{}", path.display());
        assert!(path.is_file(), "{}", path.display());
    }

    #[test]
    fn extra_arguments_reach_the_script_after_a_separator() {
        assert_eq!(npm_args(Gate::Test, &[""; 0]), ["run", "test"]);
        assert_eq!(
            npm_args(Gate::Test, &["src/a", "b.test"]),
            ["run", "test", "--", "src/a", "b.test"]
        );
    }

    #[test]
    fn the_log_names_the_arguments_the_run_had() {
        let dir = crate::test_repo::empty_dir();
        let args = npm_args(Gate::Test, &["src/a"]);
        let path = write_log(&dir.0, Gate::Test, &args, &passed("")).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let first = text.lines().next().unwrap();
        assert_eq!(first, r#"$ npm ["run", "test", "--", "src/a"]"#);
    }

    #[test]
    fn the_log_header_keeps_each_argument_whole() {
        // A space, an empty argument, a quote and a trailing backslash
        // each read back as one argument.
        let dir = crate::test_repo::empty_dir();
        let args = npm_args(Gate::Test, &["my dir", "", "a\"b", r"src\d\"]);
        let path = write_log(&dir.0, Gate::Test, &args, &passed("")).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let first = text.lines().next().unwrap();
        assert_eq!(
            first,
            r#"$ npm ["run", "test", "--", "my dir", "", "a\"b", "src\\d\\"]"#
        );
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
        let ok = passed("");
        assert_eq!(
            outcome(Gate::Test, &ok, Ok(&log(Gate::Test))),
            Ok(Outcome::Passed)
        );
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
        let f = outcome(
            Gate::Dupes,
            &failed(stdout, stderr),
            Ok(&log(Gate::Dupes)),
        )
        .unwrap_err();
        assert_eq!(
            f.summary,
            format!(
                "frontend duplication exceeds threshold (exit status: 1); \
                 full output in {}",
                log(Gate::Dupes).display()
            )
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
        let f =
            outcome(Gate::Check, &failed(stdout, ""), Ok(&log(Gate::Check)))
                .unwrap_err();
        assert!(f.items.iter().any(|i| i.ends_with("zz_type.ts:1:7")), "{f}");
        assert!(f.items.iter().any(|i| i.starts_with("Error: Type")), "{f}");
    }

    #[test]
    fn a_silent_failure_still_says_something() {
        let f = outcome(Gate::Check, &failed("", ""), Ok(&log(Gate::Check)))
            .unwrap_err();
        assert_eq!(f.items, ["exit status: 1, and no output"]);
    }

    #[test]
    fn npms_missing_script_error_is_what_gets_reported() {
        let stderr = "npm error Missing script: \"dupes\"\n";
        let f =
            outcome(Gate::Dupes, &failed("", stderr), Ok(&log(Gate::Dupes)))
                .unwrap_err();
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
