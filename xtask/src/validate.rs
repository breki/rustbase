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
/// gate `skip` names; the skipped gates' names come back too.
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
    // The gates ahead of Coverage, so a stand-in takes its slot.
    let before_coverage: Vec<&str> =
        step::position(&steps, step::names::COVERAGE)
            .map(|i| steps[..i].iter().map(Step::name).collect())
            .unwrap_or_default();
    let skipped = skip_gates(&mut steps, skip)?;
    // The stand-in is a `test` gate, so `--skip test` leaves it out
    // too. It cannot be skipped by its own name: it does not exist
    // yet when `skip_gates` checks the names.
    let stand_in_skipped = skip.iter().any(|n| n.eq_ignore_ascii_case("test"));
    if skipped.contains(&step::names::COVERAGE) && !stand_in_skipped {
        // Coverage is where the library runs the workspace's tests.
        let cfg = tasks_config();
        let at = steps
            .iter()
            .filter(|s| before_coverage.contains(&s.name()))
            .count();
        steps.insert(
            at,
            Step::new(WORKSPACE_TESTS, "test", move || {
                cargo_test::run(&cfg, cargo_test::Options::default())
                    .map_err(|f| f.to_string())?;
                Ok(String::new())
            }),
        );
    }
    Ok((steps, skipped))
}

/// The step that stands in for Coverage when it is skipped.
const WORKSPACE_TESTS: &str = "Test (workspace)";

/// Remove each gate `skip` names, matched without case against its
/// name or its subcommand, and return the removed gates' names in run
/// order. Every name is matched against the full list first, so a
/// repeat is harmless, while a name matching no gate is refused: a
/// typo must not turn into a run that skipped nothing.
fn skip_gates(
    steps: &mut Vec<Step>,
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
    let skipped = |s: &Step| skip.iter().any(|n| hit(s, n));
    let names = steps
        .iter()
        .filter(|s| skipped(s))
        .map(Step::name)
        .collect();
    steps.retain(|s| !skipped(s));
    Ok(names)
}

/// Run every gate, printing one `[N/M]` line each. `check` makes the
/// format gates read-only instead of fixing in place, for CI or
/// before partial staging, where a rewrite would sweep unrelated
/// drift into the working tree.
pub fn validate(check: bool, skip: &[String]) -> Result<(), String> {
    // Opened first, so a failure at any later point reaches the log.
    let tee = open_log(&tasks_config().root)?;
    let result = (|| {
        frontend_feature_check(
            cfg!(feature = "frontend"),
            Path::new("frontend/package.json").exists(),
        )?;
        let (steps, skipped) = steps(check, skip)?;
        if !skipped.is_empty() {
            // Said first, so a partial run is never read as a full one.
            tee.println(&format!("Skipping: {}", skipped.join(", ")));
        }
        step::run_all(steps, &tee)
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
    fn skipping_coverage_still_runs_the_workspace_tests() {
        // Coverage is where the library runs the workspace's tests,
        // so leaving it out must not leave them out.
        let names = names_without(&["coverage"]);
        let tests = names.iter().position(|n| *n == WORKSPACE_TESTS);
        let xtask = names.iter().position(|n| *n == step::names::TEST);
        assert_eq!(tests, xtask.map(|i| i + 1), "{names:?}");
        assert!(!names_without(&[]).contains(&WORKSPACE_TESTS));
    }

    #[test]
    fn skipping_coverage_and_test_runs_no_tests_and_keeps_dep_age_first() {
        // The stand-in is a `test` gate too, so `--skip test` leaves
        // it out; nothing may land ahead of the cooldown check.
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
