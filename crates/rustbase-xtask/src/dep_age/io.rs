//! The cooldown gate's unmockable I/O: the registry fetch, `git
//! show`, `cargo update --precise` (through the shared
//! `crate::process` spawn) and the clock. Kept to the
//! calls themselves so the logic around them stays testable;
//! this file is excluded from the coverage gate.
//!
//! Every command runs via the process working directory
//! (`current_dir`), not a `-C` flag, so a blanket permission
//! rule covers it.

use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::{Ecosystem, percent_encode_segment};
use crate::Config;

/// Fetch the registry metadata JSON for a package via curl.
pub(super) fn fetch_registry(
    cfg: &Config,
    ecosystem: Ecosystem,
    package: &str,
) -> Result<Value, String> {
    let package = percent_encode_segment(package);
    let url = match ecosystem {
        Ecosystem::Npm => format!("https://registry.npmjs.org/{package}"),
        Ecosystem::Cargo => {
            format!("https://crates.io/api/v1/crates/{package}")
        }
    };
    let output = Command::new("curl")
        // crates.io rejects requests without a User-Agent.
        // Bounded timeouts so a reachable-but-hanging registry
        // cannot stall the `validate` gate indefinitely: a
        // timeout is a non-zero exit -> Unavailable, which
        // leaves the dependency unchecked and fails the gate.
        .args([
            "-sSfL",
            "--connect-timeout",
            "10",
            "--max-time",
            "20",
            "-A",
            &cfg.user_agent,
            &url,
        ])
        .output()
        .map_err(|e| format!("failed to run curl (is it installed?): {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("registry request failed:\n{stderr}"));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse registry JSON: {e}"))
}

/// `git show <base>:<rel>` in `root`. `None` when the path is
/// absent at `base` or git can't run (no commit yet, not a
/// repo); the caller decides what that means.
pub(super) fn git_show(root: &Path, base: &str, rel: &str) -> Option<String> {
    String::from_utf8(git_show_bytes(root, base, rel)?).ok()
}

/// `git show <base>:<rel>` as raw bytes, for a caller that reports a
/// file that is not UTF-8 rather than taking it as absent.
pub(super) fn git_show_bytes(
    root: &Path,
    base: &str,
    rel: &str,
) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["show", &format!("{base}:{rel}")])
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

/// True when `base` names a commit in `root`'s repository, so a
/// failed `git show` there means the file did not exist yet.
pub(super) fn git_commit_exists(root: &Path, base: &str) -> bool {
    Command::new("git")
        .current_dir(root)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{base}^{{commit}}"),
        ])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Pin `name@current` to `target` via `cargo update --precise`
/// (index-only; no fetch of tarballs, no compile).
pub(super) fn cargo_update_precise(
    root: &Path,
    name: &str,
    current: &str,
    target: &str,
) -> Result<(), String> {
    let spec = format!("{name}@{current}");
    // Through the shared spawn, so this runs the same cargo (the
    // `CARGO` env var, naming the toolchain in use) as every gate.
    let out = crate::process::cargo(
        root,
        &["update", "-p", &spec, "--precise", target],
    )?;
    if out.ok {
        Ok(())
    } else {
        Err(out.stderr.trim().to_string())
    }
}

/// Today as days since the Unix epoch (UTC).
pub(super) fn today_days() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs() / 86_400).ok())
        .unwrap_or(0)
}
