//! The gates' unmockable I/O: spawning `cargo`, `npm` and other
//! tools. Kept to the spawn itself so the decisions around it stay
//! testable; this file is excluded from the coverage gate.
//!
//! Every command runs via the process working directory
//! (`current_dir`), not a `--prefix` or `-C` flag: npm resolves
//! relative `file:` specifiers against the caller's directory.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

/// A finished process, its streams decoded.
pub(crate) struct Captured {
    /// Whether it exited successfully.
    pub ok: bool,
    /// The exit status as text, for a message when it printed
    /// nothing.
    pub status: String,
    pub stdout: String,
    pub stderr: String,
}

/// Run `cargo <args>` in `root`, colour off. Prefers the `CARGO`
/// env var (set when cargo runs xtask) over a `PATH` lookup.
pub(crate) fn cargo(root: &Path, args: &[&str]) -> Result<Captured, String> {
    let bin = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(&bin);
    cmd.current_dir(root)
        .args(args)
        .env("CARGO_TERM_COLOR", "never");
    run(cmd, &bin)
}

/// Run `npm <args>` in `dir`.
pub(crate) fn npm(dir: &Path, args: &[&str]) -> Result<Captured, String> {
    let bin = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let mut cmd = Command::new(bin);
    cmd.current_dir(dir).args(args);
    run(cmd, bin)
}

/// Run `bin <args>` in `dir`. The arguments are `OsStr`, so a path
/// reaches the tool unchanged even when it is not UTF-8.
pub(crate) fn tool(
    dir: &Path,
    bin: &str,
    args: &[&OsStr],
) -> Result<Captured, String> {
    let mut cmd = Command::new(bin);
    cmd.current_dir(dir).args(args);
    run(cmd, bin)
}

fn run(mut cmd: Command, bin: &str) -> Result<Captured, String> {
    let out = cmd
        .output()
        .map_err(|e| format!("failed to run {bin}: {e}"))?;
    Ok(Captured {
        ok: out.status.success(),
        status: out.status.to_string(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}
