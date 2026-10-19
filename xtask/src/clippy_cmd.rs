use crate::helpers::run_cargo_capture;
use crate::step::Step;

/// Arguments of the workspace pass.
const CLIPPY_ARGS: &[&str] = &[
    "clippy",
    "--workspace",
    "--all-targets",
    "--",
    "-D",
    "warnings",
];

/// Maximum number of warning lines to display.
const MAX_WARNING_LINES: usize = 10;

/// Warning lines the `validate` step shows, fewer than the
/// standalone command because they sit among other steps.
const STEP_WARNING_LINES: usize = 5;

/// Run clippy with concise output.
///
/// Prints `Clippy OK` on success or `FAILED` with
/// the first few warning/error lines on failure.
pub fn clippy() -> Result<(), String> {
    let r = clippy_check()?;
    match r.error {
        None => {
            println!("Clippy OK");
            Ok(())
        }
        Some(err) => {
            eprintln!("FAILED: {err}\n");
            for line in r.items.iter().take(MAX_WARNING_LINES) {
                eprintln!("  {line}");
            }
            if r.items.len() > MAX_WARNING_LINES {
                eprintln!(
                    "  ... and {} more",
                    r.items.len() - MAX_WARNING_LINES
                );
            }
            Err(err)
        }
    }
}

/// The `validate` step; empty detail on success.
pub fn step() -> Step {
    Step::new("Clippy", "clippy", || {
        let r = clippy_check()?;
        match r.error {
            None => Ok(String::new()),
            Some(err) => {
                for line in r.items.iter().take(STEP_WARNING_LINES) {
                    eprintln!("  {line}");
                }
                Err(err)
            }
        }
    })
}

/// Result from a clippy run, for this module's `step()`.
pub struct ClippyResult {
    /// The warning/error lines (empty on success).
    pub items: Vec<String>,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Run clippy and return structured result without
/// printing.
///
/// After the workspace pass, lints `xtask` once with every
/// optional feature off and once with each feature alone. The
/// default build compiles every group, so only these passes
/// find a helper that turning a group off leaves dead, or a
/// group that needs another one.
pub fn clippy_check() -> Result<ClippyResult, String> {
    let r = run_clippy(CLIPPY_ARGS, "clippy warning(s)")?;
    if r.error.is_some() {
        return Ok(r);
    }
    let meta =
        run_cargo_capture(&["metadata", "--format-version", "1", "--no-deps"])?;
    if !meta.status.success() {
        let stderr = String::from_utf8_lossy(&meta.stderr);
        return Err(format!("cargo metadata failed:\n{stderr}"));
    }
    let meta: serde_json::Value = serde_json::from_slice(&meta.stdout)
        .map_err(|e| format!("failed to parse cargo metadata: {e}"))?;
    for set in feature_sets(&xtask_features(&meta)?) {
        let mut args = vec!["clippy", "-p", "xtask", "--all-targets"];
        args.extend(set.iter().map(String::as_str));
        args.extend(["--", "-D", "warnings"]);
        let error = format!("clippy warning(s) in xtask `{}`", set.join(" "));
        let r = run_clippy(&args, &error)?;
        if r.error.is_some() {
            return Ok(r);
        }
    }
    Ok(r)
}

/// One clippy run; `error` names the run when it fails.
fn run_clippy(args: &[&str], error: &str) -> Result<ClippyResult, String> {
    let output = run_cargo_capture(args)?;

    if output.status.success() {
        return Ok(ClippyResult {
            items: vec![],
            error: None,
        });
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let items: Vec<String> = extract_warning_lines(&stderr)
        .into_iter()
        .map(String::from)
        .collect();

    Ok(ClippyResult {
        error: Some(error.into()),
        items,
    })
}

/// The `xtask` package's optional features, sorted, from
/// `cargo metadata` output. `default` is a list of the others,
/// not a feature of its own.
///
/// # Errors
///
/// Returns an error when the metadata holds no `xtask` package,
/// so the feature passes cannot be skipped silently.
fn xtask_features(meta: &serde_json::Value) -> Result<Vec<String>, String> {
    let xtask = meta["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["name"] == "xtask")
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

/// The cargo arguments for each feature pass: all off, then
/// each feature alone.
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

/// Extract clippy diagnostics from stderr, filtering out
/// cargo/rustc summary noise, and keep the `-->`
/// source-location line that immediately follows each
/// diagnostic.
///
/// Captures three diagnostic shapes: `warning:`, `error[`
/// (coded errors), and bare `error:` -- the last is how a
/// *denied* lint is reported under `-D warnings`
/// (no `[Exxx]` code), so dropping it left a clippy failure
/// with an empty body. Without the paired `-->` line a
/// failure names the lint but not where it fired, forcing a
/// raw `cargo clippy` re-run.
fn extract_warning_lines(stderr: &str) -> Vec<&str> {
    crate::helpers::pair_with_locations(stderr, is_diagnostic_line)
}

/// True for a real clippy diagnostic line, false for the
/// cargo/rustc summary lines that share the same prefix.
fn is_diagnostic_line(line: &str) -> bool {
    // Summary noise. `") generated "` is anchored to its
    // grammar (`crate (target) generated N warnings`) so a
    // real lint message merely containing the word
    // "generated" is not dropped.
    let is_summary = line.contains("could not compile")
        || line.contains("aborting due to")
        || line.contains("build failed")
        || line.contains(") generated ")
        || line.contains(" warning emitted")
        || line.contains(" warnings emitted");
    if is_summary {
        return false;
    }
    line.starts_with("warning:")
        || line.starts_with("error[")
        || line.starts_with("error:")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn extracts_warnings_errors_and_locations() {
        let lines = extract_warning_lines(SAMPLE_STDERR);
        // Two diagnostics, each with its `-->` location; the
        // `) generated `, `could not compile`, `build
        // failed`, and `warnings emitted` summary lines are
        // filtered.
        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains("sort"));
        assert!(lines[1].contains("--> crates/rustbase/src/lib.rs:10:9"));
        assert!(lines[2].contains("E0425"));
        assert!(lines[3].contains("--> crates/rustbase/src/lib.rs:10:5"));
    }

    #[test]
    fn keeps_denied_lint_bare_error_with_location() {
        // A denied lint under `-D warnings` is reported as a
        // bare `error:` with no `[Exxx]` code. Both the
        // message and its location line must survive; the
        // `aborting due to` summary must not.
        let stderr = "\
error: this function has too many lines (111/100)
    --> crates/rustbase/src/big.rs:5:1
error: aborting due to 1 previous error";
        let lines = extract_warning_lines(stderr);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("too many lines"));
        assert!(lines[1].contains("--> crates/rustbase/src/big.rs:5:1"));
    }

    #[test]
    fn keeps_lint_message_containing_word_generated() {
        // Anchored `) generated ` filter must not drop a real
        // lint whose message merely contains "generated".
        let stderr = "warning: value assigned here is never \
            generated again";
        let lines = extract_warning_lines(stderr);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn xtask_features_reads_the_xtask_package_without_default() {
        let meta = serde_json::json!({ "packages": [
            { "name": "rustbase", "features": { "extra": [] } },
            { "name": "xtask", "features": {
                "default": ["frontend", "deploy"],
                "frontend": [],
                "deploy": [],
            } },
        ] });
        assert_eq!(xtask_features(&meta).unwrap(), vec!["deploy", "frontend"]);
    }

    #[test]
    fn xtask_features_is_empty_without_a_features_table() {
        let meta = serde_json::json!({ "packages": [
            { "name": "xtask", "features": {} },
        ] });
        assert_eq!(xtask_features(&meta).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn xtask_features_fails_without_an_xtask_package() {
        // An empty list here would skip every feature pass while
        // Clippy still reported OK.
        let meta = serde_json::json!({ "packages": [
            { "name": "rustbase", "features": {} },
        ] });
        let err = xtask_features(&meta).unwrap_err();
        assert!(err.contains("xtask"), "{err}");
        let err = xtask_features(&serde_json::json!({})).unwrap_err();
        assert!(err.contains("xtask"), "{err}");
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
    fn empty_input() {
        let lines = extract_warning_lines("");
        assert!(lines.is_empty(), "{lines:?}");
    }

    #[test]
    fn clean_output_gives_empty() {
        let stderr = "    Checking rustbase v0.2.1\n\
            Finished `dev` profile";
        let lines = extract_warning_lines(stderr);
        assert!(lines.is_empty(), "{lines:?}");
    }
}
