//! The lint gate: clippy with `-D warnings` over the workspace, then
//! over the `xtask` package with every optional feature off and with
//! each feature alone.
//!
//! The default build compiles every feature group, so only those
//! extra passes find a helper that turning a group off leaves dead,
//! or a group that needs another one.

use serde_json::Value;

use crate::process::{self, Captured};
use crate::text::{is_cargo_diagnostic, pair_with_locations};
use crate::{Config, Failure, TOOLING_CRATE};

/// Arguments of the workspace pass.
const CLIPPY_ARGS: &[&str] = &[
    "clippy",
    "--workspace",
    "--all-targets",
    "--",
    "-D",
    "warnings",
];

/// Lints the workspace and the `xtask` feature combinations in the
/// workspace root, stopping at the first pass that fails.
///
/// # Errors
///
/// The failing pass's diagnostics, or why cargo could not run, or a
/// `cargo metadata` that lists no `xtask` package.
pub fn check(cfg: &Config) -> Result<(), Failure> {
    run_passes(|args| process::cargo(&cfg.root, args))
}

/// [`check`] with the cargo spawn injected.
fn run_passes(
    mut cargo: impl FnMut(&[&str]) -> Result<Captured, String>,
) -> Result<(), Failure> {
    judge(&cargo(CLIPPY_ARGS)?, "clippy warning(s)")?;
    let meta = cargo(&["metadata", "--format-version", "1", "--no-deps"])?;
    if !meta.ok {
        return Err(format!("cargo metadata failed:\n{}", meta.stderr).into());
    }
    let meta: Value = serde_json::from_str(&meta.stdout)
        .map_err(|e| format!("failed to parse cargo metadata: {e}"))?;
    for set in feature_sets(&xtask_features(&meta)?) {
        let mut args = vec!["clippy", "-p", TOOLING_CRATE, "--all-targets"];
        args.extend(set.iter().map(String::as_str));
        args.extend(["--", "-D", "warnings"]);
        let summary = format!("clippy warning(s) in xtask `{}`", set.join(" "));
        judge(&cargo(&args)?, &summary)?;
    }
    Ok(())
}

/// One finished clippy pass; `summary` names it when it fails.
fn judge(out: &Captured, summary: &str) -> Result<(), Failure> {
    if out.ok {
        return Ok(());
    }
    let items = pair_with_locations(&out.stderr, is_cargo_diagnostic)
        .into_iter()
        .map(String::from)
        .collect();
    Err(Failure::new(summary, items))
}

/// The `xtask` package's optional features, sorted, from `cargo
/// metadata` output. `default` is a list of the others, not a
/// feature of its own.
///
/// # Errors
///
/// When the metadata holds no `xtask` package, so the feature passes
/// cannot be skipped silently.
fn xtask_features(meta: &Value) -> Result<Vec<String>, String> {
    let xtask = meta["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["name"] == TOOLING_CRATE)
        .ok_or("cargo metadata lists no `xtask` package")?;
    let mut features: Vec<String> = xtask["features"]
        .as_object()
        .into_iter()
        .flat_map(serde_json::Map::keys)
        .filter(|k| *k != "default")
        .cloned()
        .collect();
    features.sort();
    Ok(features)
}

/// The cargo arguments for each feature pass: all off, then each
/// feature alone.
fn feature_sets(features: &[String]) -> Vec<Vec<String>> {
    let off = || vec!["--no-default-features".to_owned()];
    let mut sets = vec![off()];
    for f in features {
        let mut set = off();
        set.extend(["--features".to_owned(), f.clone()]);
        sets.push(set);
    }
    sets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(ok: bool, stdout: &str, stderr: &str) -> Captured {
        Captured {
            ok,
            status: String::new(),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    const SAMPLE_STDERR: &str = "\
warning: used `sort` on primitive type `str`
    --> crates/rustbase/src/lib.rs:10:9
warning: `rustbase` (bin \"rustbase\" test) \
generated 1 warning (1 duplicate)
error[E0425]: cannot find value `x`
    --> crates/rustbase/src/lib.rs:10:5
error: could not compile `rustbase`
warning: build failed, waiting for other jobs
warning: 2 warnings emitted";

    #[test]
    fn a_failing_pass_reports_its_diagnostics_with_locations() {
        let f = judge(&captured(false, "", SAMPLE_STDERR), "clippy warning(s)")
            .unwrap_err();
        assert_eq!(f.summary, "clippy warning(s)");
        assert_eq!(f.items.len(), 4, "{:?}", f.items);
        assert!(f.items[0].contains("sort"));
        assert!(f.items[1].contains("lib.rs:10:9"));
        assert!(f.items[2].contains("E0425"));
        assert!(f.items[3].contains("lib.rs:10:5"));
    }

    #[test]
    fn a_denied_lint_keeps_its_bare_error_and_location() {
        let stderr = "\
error: this function has too many lines (111/100)
    --> crates/rustbase/src/big.rs:5:1
error: aborting due to 1 previous error";
        let f = judge(&captured(false, "", stderr), "x").unwrap_err();
        assert_eq!(f.items.len(), 2);
        assert!(f.items[0].contains("too many lines"));
    }

    fn meta() -> String {
        serde_json::json!({ "packages": [
            { "name": "rustbase", "features": { "extra": [] } },
            { "name": "xtask", "features": {
                "default": ["frontend", "deploy"],
                "frontend": [],
                "deploy": [],
            } },
        ] })
        .to_string()
    }

    #[test]
    fn xtask_features_reads_the_xtask_package_without_default() {
        let meta: Value = serde_json::from_str(&meta()).unwrap();
        assert_eq!(xtask_features(&meta).unwrap(), vec!["deploy", "frontend"]);
    }

    #[test]
    fn xtask_features_fails_without_an_xtask_package() {
        // An empty list here would skip every feature pass while
        // Clippy still reported OK.
        let meta = serde_json::json!({ "packages": [
            { "name": "rustbase", "features": {} },
        ] });
        assert!(xtask_features(&meta).unwrap_err().contains("xtask"));
        assert!(xtask_features(&serde_json::json!({})).is_err());
    }

    #[test]
    fn feature_sets_are_all_off_then_each_alone() {
        let sets = feature_sets(&["deploy".into(), "frontend".into()]);
        assert_eq!(
            sets,
            vec![
                vec!["--no-default-features"],
                vec!["--no-default-features", "--features", "deploy"],
                vec!["--no-default-features", "--features", "frontend"],
            ]
        );
    }

    #[test]
    fn every_pass_runs_when_all_are_clean() {
        let mut seen = Vec::new();
        let m = meta();
        let r = run_passes(|args| {
            seen.push(args.join(" "));
            Ok(captured(
                true,
                if args[0] == "metadata" { &m } else { "" },
                "",
            ))
        });
        assert_eq!(r, Ok(()));
        assert_eq!(seen.len(), 5, "{seen:?}");
        assert!(seen[4].contains(
            "-p xtask --all-targets --no-default-features --features frontend"
        ));
    }

    #[test]
    fn the_first_failing_pass_stops_the_run_and_names_itself() {
        let m = meta();
        let f = run_passes(|args| {
            let fail = args.contains(&"deploy");
            let stdout = if args[0] == "metadata" {
                m.as_str()
            } else {
                ""
            };
            Ok(captured(
                !fail,
                stdout,
                "warning: dead code\n  --> x.rs:1:1",
            ))
        })
        .unwrap_err();
        assert!(f.summary.contains("--features deploy"), "{}", f.summary);
        assert_eq!(f.items.len(), 2);
    }

    #[test]
    fn a_metadata_failure_is_reported() {
        let f = run_passes(|args| {
            Ok(captured(args[0] != "metadata", "", "no manifest"))
        })
        .unwrap_err();
        assert!(f.summary.contains("cargo metadata failed"), "{}", f.summary);
        let f = run_passes(|_| Ok(captured(true, "not json", ""))).unwrap_err();
        assert!(f.summary.contains("parse cargo metadata"), "{}", f.summary);
    }
}
