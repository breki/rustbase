use std::path::Path;

use rustbase_xtask::cargo_test;
use rustbase_xtask::step::{self, Step};
use rustbase_xtask::tee::{OpenError, Tee};

use crate::helpers::tasks_config;
#[cfg(feature = "frontend")]
use crate::{frontend_check, frontend_dupes, frontend_fmt, frontend_test};

/// The library's standard gates (`step::standard`, which gives their
/// order), with this project's frontend gates after Deny (three static
/// ones, then the frontend tests, ahead of the Rust tests), less any
/// gate `skip` names; the skipped gates' names come back too. When
/// Coverage is skipped but Test is not, a `Test (workspace)` step takes
/// Coverage's slot and the Test gate goes too, unlisted: the stand-in
/// runs its tests. A name matching no gate, or a set leaving none, is
/// refused.
///
/// The `[N/M]` numbering follows whichever gates are compiled in.
fn steps(
    check: bool,
    skip: &[String],
) -> Result<(Vec<Step>, Vec<&'static str>), String> {
    let mut steps = step::standard(&tasks_config(), check);
    #[cfg(feature = "frontend")]
    step::insert_after(
        &mut steps,
        step::names::DENY,
        [
            frontend_fmt::step(check),
            frontend_check::step(),
            frontend_dupes::step(),
            frontend_test::step(),
        ],
    )?;
    let skipped = skipped_gates(&steps, skip)?;
    if skipped.contains(&step::names::COVERAGE)
        && !skipped.contains(&step::names::TEST)
    {
        // Coverage runs every crate's tests but xtask's, and the Test
        // gate runs xtask's. The stand-in runs them all, so it replaces
        // both. With Test skipped too, it is never added.
        if let Some(i) = step::position(&steps, step::names::COVERAGE) {
            steps[i] = workspace_tests();
        }
        steps.retain(|s| s.name() != step::names::TEST);
    }
    steps.retain(|s| !skipped.contains(&s.name()));
    if steps.is_empty() {
        // It would end "Validate OK" having run nothing.
        return Err("--skip leaves out every gate; nothing to validate".into());
    }
    Ok((steps, skipped))
}

/// Every crate's tests, xtask's included, run where Coverage would
/// have run the rest.
fn workspace_tests() -> Step {
    let cfg = tasks_config();
    Step::new(WORKSPACE_TESTS, "test", move || {
        cargo_test::run(&cfg, cargo_test::Options::default())
            .map_err(|f| f.to_string())?;
        Ok(String::new())
    })
}

/// The step that stands in for Coverage when it is skipped.
const WORKSPACE_TESTS: &str = "Test (workspace)";

/// The names of the gates `skip` names, matched without case against
/// each gate's name or subcommand, in run order. A name matching no
/// gate is refused: a typo must not turn into a run that skipped
/// nothing. The gates are walked, keeping each that any name hits, so
/// `--skip coverage --skip Coverage` reports Coverage once.
fn skipped_gates(
    steps: &[Step],
    skip: &[String],
) -> Result<Vec<&'static str>, String> {
    let hit = |s: &Step, name: &str| {
        s.name().eq_ignore_ascii_case(name)
            || s.cmd().eq_ignore_ascii_case(name)
    };
    if let Some(name) = skip.iter().find(|n| !steps.iter().any(|s| hit(s, n))) {
        let gates: Vec<&str> = steps.iter().map(Step::cmd).collect();
        return Err(format!(
            "--skip {name}: no such gate (gates: {})",
            gates.join(", ")
        ));
    }
    Ok(steps
        .iter()
        .filter(|s| skip.iter().any(|n| hit(s, n)))
        .map(Step::name)
        .collect())
}

/// Run every gate except those `skip` names, printing one `[N/M]` line
/// each; a partial run names its skipped gates before the first step
/// and after a passing run's last line. `check` makes the format gates
/// read-only instead of fixing in place, for CI or before partial
/// staging, where a rewrite would sweep unrelated drift into the
/// working tree.
pub fn validate(check: bool, skip: &[String]) -> Result<(), String> {
    // Opened first, so a failure at any later point reaches the log.
    let tee = open_log(&tasks_config().root)?;
    let result = (|| {
        frontend_feature_check(
            cfg!(feature = "frontend"),
            Path::new("frontend/package.json").exists(),
        )?;
        let (steps, skipped) = steps(check, skip)?;
        // Said first and last, so neither the head nor the tail of a
        // partial run reads as a full one.
        let names = skipped.join(", ");
        if !skipped.is_empty() {
            tee.println(&format!("Skipping: {names}"));
        }
        step::run_all(steps, &tee)?;
        if !skipped.is_empty() {
            tee.println(&format!("Partial run: skipped {names}"));
        }
        Ok(())
    })();
    record_failure(&tee, &result);
    result
}

/// `target/validate.log`. While a first run holds it, a second run is
/// refused: it could not write the log, which would keep the first
/// run's output, and a reader would take that as the second run's
/// result. Any other failure to open it warns and falls back to
/// terminal-only output, since the log is a side record.
fn open_log(root: &Path) -> Result<Tee, String> {
    match Tee::open(root, "validate") {
        Ok(tee) => Ok(tee),
        Err(OpenError::Held(path)) => Err(format!(
            "another validate is running in this checkout ({} is held); \
             wait for it to finish",
            path.display()
        )),
        Err(e) => {
            eprintln!("warning: {e}; this run is not logged");
            Ok(Tee::terminal())
        }
    }
}

/// Write a failure to the log as `main` prints it.
fn record_failure(tee: &Tee, result: &Result<(), String>) {
    if let Err(e) = result {
        tee.record(&crate::error_line(e));
    }
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
        for step in steps(false, &[]).unwrap().0 {
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
    fn frontend_steps_follow_the_feature_and_sit_after_deny() {
        // The frontend gates are in the list exactly when the
        // feature compiles them in, so the `[N/M]` total counts
        // only gates that exist.
        let all = steps(false, &[]).unwrap().0;
        let names: Vec<&str> = all.iter().map(Step::name).collect();
        let deny = names.iter().position(|n| *n == step::names::DENY).unwrap();
        let first = names.iter().position(|n| n.starts_with("Frontend"));
        if cfg!(feature = "frontend") {
            assert_eq!(first, Some(deny + 1), "{names:?}");
        } else {
            assert_eq!(first, None, "{names:?}");
        }
    }

    fn names_without(skip: &[&str]) -> Vec<&'static str> {
        let skip: Vec<String> = skip.iter().map(|s| (*s).into()).collect();
        steps(false, &skip)
            .unwrap()
            .0
            .iter()
            .map(Step::name)
            .collect()
    }

    #[test]
    fn skip_leaves_the_named_gates_out_ignoring_case() {
        let names = names_without(&["coverage", "DUPLICATION"]);
        assert!(!names.contains(&step::names::COVERAGE), "{names:?}");
        assert!(!names.contains(&step::names::DUPLICATION), "{names:?}");
        assert!(names.contains(&step::names::CLIPPY), "{names:?}");
    }

    #[test]
    fn skip_also_matches_a_gate_by_its_subcommand() {
        let names = names_without(&["dupes"]);
        assert!(!names.contains(&step::names::DUPLICATION), "{names:?}");
    }

    #[test]
    fn skipping_every_gate_is_refused() {
        // A run with no gate would end "Validate OK" having run nothing.
        let all: Vec<String> = steps(false, &[])
            .unwrap()
            .0
            .iter()
            .map(|s| s.cmd().to_owned())
            .collect();
        let err = steps(false, &all).err().unwrap();
        assert!(err.contains("every gate"), "{err}");
    }

    #[test]
    fn skipping_an_unknown_gate_is_refused() {
        // A typo would otherwise skip nothing and read as a full run.
        let err = steps(false, &["covrage".into()]).err().unwrap();
        assert!(err.contains("covrage"), "{err}");
    }

    #[test]
    fn skip_tolerates_a_repeated_name_in_any_case() {
        let names = names_without(&["coverage", "Coverage"]);
        assert!(!names.contains(&step::names::COVERAGE), "{names:?}");
    }

    #[test]
    fn skip_reports_the_gates_it_removed_by_name() {
        let skip = ["COVERAGE".to_owned(), "dupes".to_owned()];
        let (_, skipped) = steps(false, &skip).unwrap();
        assert_eq!(
            skipped,
            [step::names::DUPLICATION, step::names::COVERAGE],
            "in run order, as the [N/M] lines name them"
        );
    }

    #[test]
    fn skipping_coverage_runs_every_crates_tests_once_in_its_place() {
        // Coverage is where the library runs the workspace's tests, so
        // a stand-in takes its slot. It runs xtask's tests too, so the
        // separate xtask step goes rather than running them twice.
        let full = names_without(&[]);
        let expected: Vec<&str> = full
            .iter()
            .filter(|n| **n != step::names::TEST)
            .map(|n| {
                if *n == step::names::COVERAGE {
                    WORKSPACE_TESTS
                } else {
                    n
                }
            })
            .collect();
        assert_eq!(names_without(&["coverage"]), expected);
    }

    #[test]
    fn skipping_coverage_and_the_xtask_tests_by_name_runs_no_rust_tests() {
        // The same as `--skip test`: one matching rule for name and
        // subcommand alike.
        let names = names_without(&["coverage", "Test (xtask only)"]);
        assert!(!names.contains(&WORKSPACE_TESTS), "{names:?}");
        assert!(!names.contains(&step::names::TEST), "{names:?}");
    }

    #[test]
    fn skipping_coverage_and_test_runs_no_rust_tests_and_keeps_dep_age_first() {
        // With Test skipped the stand-in is never added, and no step
        // lands ahead of Dep-age, which vets what the tests compile.
        let names = names_without(&["coverage", "test"]);
        assert_eq!(names.first(), Some(&step::names::DEP_AGE), "{names:?}");
        assert!(!names.contains(&WORKSPACE_TESTS), "{names:?}");
    }

    #[test]
    fn the_help_carries_no_program_description() {
        // `about = None` on `Cli` keeps the doc comments of `Cli`,
        // `XCommand` and the flattened `LibCommand` out of `--help`.
        assert!(crate::Cli::command().get_about().is_none());
    }

    #[test]
    fn a_log_that_cannot_open_falls_back_to_the_terminal() {
        let root = std::env::temp_dir()
            .join(format!("xtask-validate-nolog-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("target"), "a file, not a dir").unwrap();
        let tee = open_log(&root);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(tee.unwrap().path(), None);
    }

    #[test]
    fn a_second_run_while_one_holds_the_log_is_refused() {
        // Its fallback would leave the first run's result in the log,
        // read as this run's.
        let root = std::env::temp_dir()
            .join(format!("xtask-validate-held-{}", std::process::id()));
        let first = Tee::open(&root, "validate").unwrap();
        let e = open_log(&root).unwrap_err();
        drop(first);
        let _ = std::fs::remove_dir_all(&root);
        assert!(e.contains("another validate is running"), "{e}");
    }

    #[test]
    fn a_failure_is_logged_as_main_prints_it() {
        let root = std::env::temp_dir()
            .join(format!("xtask-validate-log-{}", std::process::id()));
        let tee = Tee::open(&root, "validate").unwrap();
        record_failure(&tee, &Ok(()));
        record_failure(&tee, &Err("boom".into()));
        drop(tee);
        let log = std::fs::read_to_string(root.join("target/validate.log"));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(log.unwrap(), format!("{}\n", crate::error_line("boom")));
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
}
