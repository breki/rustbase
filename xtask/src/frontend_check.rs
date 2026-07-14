//! Runs `svelte-check` against the frontend TypeScript +
//! Svelte sources. Skipped gracefully when the frontend
//! is absent (the template's web crate is optional).

use std::path::Path;
use std::process::Command;

/// Outcome of a frontend check run.
pub struct FrontendCheckResult {
    /// Human-readable detail for the step line.
    pub detail: String,
    /// Error message if the check failed.
    pub error: Option<String>,
    /// True when the check was skipped (no frontend).
    pub skipped: bool,
}

/// What to do about the frontend, given what is on disk.
#[derive(Debug, PartialEq, Eq)]
enum FrontendAction {
    /// No frontend in this project (CLI-only). Skip, pass.
    Skip,
    /// Frontend exists but its deps are not installed. This
    /// must be an error, not a skip: a skip that exits 0 is
    /// indistinguishable from a pass, so an uninstalled (or
    /// unreachable-from-cwd) frontend would silently go
    /// unchecked while reading green.
    NotInstalled,
    /// Frontend is present and installed. Run the check.
    Run,
}

/// Decide the action from the two on-disk facts. Pure, so
/// the skip-vs-error boundary is unit-tested without npm.
fn classify(has_package_json: bool, has_node_modules: bool) -> FrontendAction {
    match (has_package_json, has_node_modules) {
        (false, _) => FrontendAction::Skip,
        (true, false) => FrontendAction::NotInstalled,
        (true, true) => FrontendAction::Run,
    }
}

/// Standalone `cargo xtask frontend-check` entry point.
pub fn frontend_check_cmd() -> Result<(), String> {
    let r = frontend_check()?;
    match r.error {
        None if r.skipped => {
            println!("Frontend check skipped: {}", r.detail);
            Ok(())
        }
        None => {
            println!("Frontend check OK");
            Ok(())
        }
        Some(e) => Err(e),
    }
}

/// Run `npm run check` in `frontend/`.
///
/// Skips (pass) only when there is genuinely no frontend
/// (`frontend/package.json` absent -- a CLI-only project).
/// When the frontend exists but `frontend/node_modules` is
/// not installed, returns an error instead of skipping, so
/// a missing install can never be mistaken for a pass.
pub fn frontend_check() -> Result<FrontendCheckResult, String> {
    let has_pkg = Path::new("frontend/package.json").exists();
    let has_modules = Path::new("frontend/node_modules").exists();
    match classify(has_pkg, has_modules) {
        FrontendAction::Skip => {
            return Ok(FrontendCheckResult {
                detail: "no frontend".into(),
                error: None,
                skipped: true,
            });
        }
        FrontendAction::NotInstalled => {
            return Err("frontend/ exists but node_modules is \
                 missing -- run `npm --prefix frontend install`"
                .into());
        }
        FrontendAction::Run => {}
    }

    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let output = Command::new(npm)
        .args(["--prefix", "frontend", "run", "check"])
        .output()
        .map_err(|e| format!("failed to run {npm}: {e}"))?;

    if output.status.success() {
        Ok(FrontendCheckResult {
            detail: String::new(),
            error: None,
            skipped: false,
        })
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}{stderr}");
        for line in combined.lines().take(10) {
            eprintln!("  {line}");
        }
        Ok(FrontendCheckResult {
            detail: String::new(),
            error: Some("svelte-check reported errors".into()),
            skipped: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_package_json_skips() {
        assert_eq!(classify(false, false), FrontendAction::Skip);
        // A stray node_modules without package.json is still
        // "no frontend".
        assert_eq!(classify(false, true), FrontendAction::Skip);
    }

    #[test]
    fn package_json_without_modules_is_error() {
        // The case that used to skip-as-pass: frontend exists
        // but is not installed.
        assert_eq!(classify(true, false), FrontendAction::NotInstalled);
    }

    #[test]
    fn installed_frontend_runs() {
        assert_eq!(classify(true, true), FrontendAction::Run);
    }
}
