use std::path::Path;

use rustbase_xtask::step::{self, Step};
use rustbase_xtask::tee::Tee;

use crate::helpers::tasks_config;
#[cfg(feature = "frontend")]
use crate::{frontend_check, frontend_dupes, frontend_fmt, frontend_test};

/// The library's standard gates (`step::standard`, which gives their
/// order), with this project's frontend gates after Deny: three
/// static ones, then the frontend tests, ahead of the Rust tests.
///
/// The `[N/M]` numbering follows whichever gates are compiled in.
// Without the frontend feature nothing is inserted, so nothing fails.
#[cfg_attr(not(feature = "frontend"), allow(clippy::unnecessary_wraps))]
fn steps(check: bool) -> Result<Vec<Step>, String> {
    #[cfg_attr(not(feature = "frontend"), allow(unused_mut))]
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
    Ok(steps)
}

/// Run every gate, printing one `[N/M]` line each. `check` makes the
/// format gates read-only instead of fixing in place, for CI or
/// before partial staging, where a rewrite would sweep unrelated
/// drift into the working tree.
pub fn validate(check: bool) -> Result<(), String> {
    frontend_feature_check(
        cfg!(feature = "frontend"),
        Path::new("frontend/package.json").exists(),
    )?;
    let tee = Tee::open(&tasks_config().root, "validate")?;
    step::run_all(steps(check)?, &tee)
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
        for step in steps(false).unwrap() {
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
        let all = steps(false).unwrap();
        let names: Vec<&str> = all.iter().map(Step::name).collect();
        let deny = names.iter().position(|n| *n == step::names::DENY).unwrap();
        let first = names.iter().position(|n| n.starts_with("Frontend"));
        if cfg!(feature = "frontend") {
            assert_eq!(first, Some(deny + 1), "{names:?}");
        } else {
            assert_eq!(first, None, "{names:?}");
        }
    }

    #[test]
    fn the_help_carries_no_program_description() {
        // `about = None` on `Cli` keeps the doc comments of `Cli`,
        // `XCommand` and the flattened `LibCommand` out of `--help`.
        assert!(crate::Cli::command().get_about().is_none());
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
