use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

/// Width of the step name column (including dots).
const STEP_NAME_WIDTH: usize = 14;

/// Format a validation step result line as a string.
///
/// Returns: `[1/5] Fmt........... OK (0.3s)`
pub fn format_step(
    step: usize,
    total: usize,
    name: &str,
    status: &str,
    detail: &str,
) -> String {
    let dots = ".".repeat(STEP_NAME_WIDTH.saturating_sub(name.len()));
    if detail.is_empty() {
        format!("[{step}/{total}] {name}{dots} {status}")
    } else {
        format!(
            "[{step}/{total}] {name}{dots} {status} \
             ({detail})"
        )
    }
}

/// Print a validation step result line to stdout.
///
/// Produces: `[1/5] Fmt........... OK (0.3s)`
pub fn step_output(
    step: usize,
    total: usize,
    name: &str,
    status: &str,
    detail: &str,
) {
    println!("{}", format_step(step, total, name, status, detail));
}

/// Format elapsed time as a human-readable string.
pub fn elapsed_str(start: Instant) -> String {
    let secs = start.elapsed().as_secs_f64();
    format!("{secs:.1}s")
}

/// Format a byte count as a human-readable string with
/// a binary unit suffix (`B`, `KiB`, `MiB`, `GiB`,
/// `TiB`). One decimal place above `B`.
pub fn fmt_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    // Cache sizes routinely sum into the tens-of-GB
    // range, well under f64's 2^52 mantissa headroom,
    // so precision loss is not a real concern here.
    #[allow(clippy::cast_precision_loss)]
    let mut v = bytes as f64;
    let mut idx = 0;
    while v >= 1024.0 && idx + 1 < UNITS.len() {
        v /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{v:.1} {}", UNITS[idx])
    }
}

/// Recursive byte sum for a path. Symlinks are not
/// followed and contribute zero bytes -- defends
/// against symlink-loop stack blow-up and against
/// attributing symlink-target sizes to the source.
///
/// Errors at every level carry the failing path so
/// downstream warnings can name the actual culprit
/// rather than the top-level entry being walked.
pub fn dir_size(path: &Path) -> Result<u64, String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("symlink_metadata {}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        return Ok(0);
    }
    if meta.is_file() {
        return Ok(meta.len());
    }
    let mut total = 0u64;
    let entries = fs::read_dir(path)
        .map_err(|e| format!("read_dir {}: {e}", path.display()))?;
    for entry in entries {
        let entry = entry
            .map_err(|e| format!("entry under {}: {e}", path.display()))?;
        match dir_size(&entry.path()) {
            Ok(n) => total += n,
            Err(e) => eprintln!("warning: {e} (skipping)"),
        }
    }
    Ok(total)
}

/// Per-test scratch directory under the system temp.
/// PID + thread id + atomic counter keep parallel test
/// runs from colliding without adding a `tempfile`
/// dependency. Cleanup is the caller's responsibility
/// (best-effort `remove_dir_all` at end of test).
#[cfg(test)]
pub(crate) fn temp_scratch(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let tid = format!("{:?}", std::thread::current().id());
    let tid_clean: String =
        tid.chars().filter(char::is_ascii_alphanumeric).collect();
    let dir = std::env::temp_dir()
        .join(format!("rustbase-xtask-{label}-{pid}-{tid_clean}-{seq}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Resolve the cargo binary path. Prefers the `CARGO`
/// env var (set by cargo when running xtask) over a
/// PATH lookup.
pub fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

/// Resolve the workspace root (the parent of the xtask
/// crate directory).
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate always lives under workspace root")
        .to_path_buf()
}

/// Run a cargo command and capture its output.
///
/// Strips ANSI codes via `CARGO_TERM_COLOR=never`.
pub fn run_cargo_capture(args: &[&str]) -> Result<Output, String> {
    let bin = cargo_bin();
    Command::new(&bin)
        .args(args)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .map_err(|e| format!("failed to run {bin}: {e}"))
}

/// Run a cargo command, streaming output to the
/// terminal. Used for `--verbose` mode and fmt.
pub fn run_cargo_stream(args: &[&str]) -> Result<(), String> {
    let bin = cargo_bin();
    let status = Command::new(&bin)
        .args(args)
        .status()
        .map_err(|e| format!("failed to run {bin}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        match status.code() {
            Some(code) => Err(format!("{bin} exited with {code}")),
            None => Err(format!("{bin} terminated by signal")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_step_with_detail() {
        assert_eq!(
            format_step(1, 5, "Fmt", "OK", "0.3s"),
            "[1/5] Fmt........... OK (0.3s)"
        );
    }

    #[test]
    fn format_step_without_detail() {
        assert_eq!(
            format_step(2, 5, "Clippy", "FAILED", ""),
            "[2/5] Clippy........ FAILED"
        );
    }

    #[test]
    fn format_step_long_name_no_overflow() {
        let result = format_step(1, 1, "VeryLongStepName", "OK", "");
        assert_eq!(result, "[1/1] VeryLongStepName OK");
    }

    #[test]
    fn workspace_root_contains_cargo_toml() {
        let root = workspace_root();
        assert!(
            root.join("Cargo.toml").is_file(),
            "workspace root should contain Cargo.toml: {}",
            root.display()
        );
    }

    #[test]
    fn fmt_bytes_under_kib() {
        assert_eq!(fmt_bytes(0), "0 B");
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(1023), "1023 B");
    }

    #[test]
    fn fmt_bytes_scaling() {
        assert_eq!(fmt_bytes(1024), "1.0 KiB");
        assert_eq!(fmt_bytes(1536), "1.5 KiB");
        assert_eq!(fmt_bytes(1024 * 1024), "1.0 MiB");
        assert_eq!(fmt_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }

    #[test]
    fn elapsed_str_format() {
        let start = Instant::now();
        let result = elapsed_str(start);
        assert!(result.ends_with('s'), "should end with 's': {result}");
        assert!(result.contains('.'), "should have decimal: {result}");
    }
}
