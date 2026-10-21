//! The gates' unmockable I/O: spawning `cargo`, `npm` and other
//! tools. Kept to the spawn itself so the decisions around it stay
//! testable; this file is excluded from the coverage gate.
//!
//! Every command runs via the process working directory
//! (`current_dir`), not a `--prefix` or `-C` flag: npm resolves
//! relative `file:` specifiers against the caller's directory.

use std::ffi::OsStr;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::deadline::{self, Waited};

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

/// Run `cargo <args>` in `root`, colour off.
pub(crate) fn cargo(root: &Path, args: &[&str]) -> Result<Captured, String> {
    cargo_env(root, args, &[])
}

/// The cargo to run: the `CARGO` env var (set when cargo runs
/// xtask, naming the toolchain in use) over a `PATH` lookup.
fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

/// [`cargo`] with extra environment variables, for settings cargo
/// takes only from the environment (such as `RUSTDOCFLAGS`).
pub(crate) fn cargo_env(
    root: &Path,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<Captured, String> {
    let bin = cargo_bin();
    let mut cmd = Command::new(&bin);
    cmd.current_dir(root)
        .args(args)
        .env("CARGO_TERM_COLOR", "never")
        .envs(env.iter().copied());
    run(cmd, &bin)
}

/// Run `cargo <args>` in `root` with its output going straight to
/// the terminal, for a command whose output is the point (`fmt`, a
/// verbose test run).
pub(crate) fn cargo_stream(root: &Path, args: &[&str]) -> Result<(), String> {
    let bin = cargo_bin();
    let status = Command::new(&bin)
        .current_dir(root)
        .args(args)
        .status()
        .map_err(|e| format!("failed to run {bin}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{bin} {} failed ({status})", args.join(" ")))
    }
}

/// [`cargo`], stopped after `limit`.
pub(crate) fn cargo_within(
    root: &Path,
    args: &[&str],
    limit: Duration,
) -> Result<Captured, String> {
    let bin = cargo_bin();
    let mut cmd = Command::new(&bin);
    cmd.current_dir(root)
        .args(args)
        .env("CARGO_TERM_COLOR", "never");
    run_within(cmd, &bin, limit)
}

/// The npm executable: a `.cmd` shim on Windows.
fn npm_bin() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

/// Run `npm <args>` in `dir`.
#[cfg(feature = "frontend")]
pub(crate) fn npm(dir: &Path, args: &[&str]) -> Result<Captured, String> {
    let bin = npm_bin();
    let mut cmd = Command::new(bin);
    cmd.current_dir(dir).args(args);
    run(cmd, bin)
}

/// [`npm`], stopped after `limit`.
pub(crate) fn npm_within(
    dir: &Path,
    args: &[&str],
    limit: Duration,
) -> Result<Captured, String> {
    let bin = npm_bin();
    let mut cmd = Command::new(bin);
    cmd.current_dir(dir).args(args);
    run_within(cmd, bin, limit)
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

/// How often a time-limited run checks whether its child is done.
const POLL: Duration = Duration::from_millis(100);

impl deadline::Poll for Child {
    type Status = ExitStatus;
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Child::try_wait(self)
    }
    fn kill(&mut self) -> io::Result<()> {
        Child::kill(self)
    }
}

/// Run `cmd`, killing it once `limit` has passed. Only the direct
/// child is killed: a process it started (cargo's `cargo-audit`,
/// npm's `node`) can outlive it and hold the output pipes open, so a
/// timed-out run returns without waiting for its readers.
fn run_within(
    mut cmd: Command,
    bin: &str,
    limit: Duration,
) -> Result<Captured, String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run {bin}: {e}"))?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let waited = deadline::wait_until(&mut child, limit, POLL)
        .map_err(|e| format!("failed to wait for {bin}: {e}"))?;
    match waited {
        Waited::Exited(status) => Ok(Captured {
            ok: status.success(),
            status: status.to_string(),
            stdout: joined(stdout),
            stderr: joined(stderr),
        }),
        Waited::TimedOut => {
            // Reap the killed child; its readers are left behind.
            let _ = child.wait();
            Err(format!(
                "{bin} did not finish within {limit:?} and was stopped"
            ))
        }
    }
}

/// Read a child's pipe to the end on a thread of its own, so a full
/// pipe cannot stall the child while the caller waits on it.
fn drain(pipe: Option<impl Read + Send + 'static>) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// A drained pipe's text; empty if its reader panicked.
fn joined(reader: JoinHandle<String>) -> String {
    reader.join().unwrap_or_default()
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
