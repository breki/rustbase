use std::path::PathBuf;
use std::process::{Command, Output};

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

/// This project's settings for the shared `rustbase-xtask`
/// gates. A derived project changes the name here (and with it
/// the `<NAME>_DEP_AGE_ALLOW` variable and the user agent).
pub fn tasks_config() -> rustbase_xtask::Config {
    rustbase_xtask::Config::for_project("rustbase", workspace_root())
}

/// Run a cargo command and capture its output.
///
/// Strips ANSI codes via `CARGO_TERM_COLOR=never`.
pub fn run_cargo_capture(args: &[&str]) -> Result<Output, String> {
    run_cargo_capture_env(args, &[])
}

/// [`run_cargo_capture`] with extra environment variables, for
/// settings cargo takes only from the environment (such as
/// `RUSTDOCFLAGS`).
pub fn run_cargo_capture_env(
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<Output, String> {
    let bin = cargo_bin();
    let mut cmd = Command::new(&bin);
    cmd.args(args).env("CARGO_TERM_COLOR", "never");
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.output()
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

/// Keep every line for which `keep` returns true, plus the
/// `-->` source-location line that immediately follows a
/// kept line, so a diagnostic is surfaced together with
/// where it fired; without the paired location a failure
/// names the lint/error but not the `file:line`, forcing a
/// raw re-run. Only the location line *immediately* after a
/// kept line is taken, so a `-->` following a filtered line
/// is never wrongly attached.
pub fn pair_with_locations<F>(stderr: &str, keep: F) -> Vec<&str>
where
    F: Fn(&str) -> bool,
{
    let mut out: Vec<&str> = Vec::new();
    let mut prev_kept = false;
    for line in stderr.lines() {
        if keep(line) {
            out.push(line);
            prev_kept = true;
        } else if prev_kept && line.trim_start().starts_with("-->") {
            out.push(line);
            prev_kept = false;
        } else {
            prev_kept = false;
        }
    }
    out
}

/// Return the last `n` non-empty lines of `stderr`.
pub fn stderr_tail(stderr: &str, n: usize) -> Vec<&str> {
    let mut lines: Vec<&str> =
        stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(n);
    lines.drain(..start);
    lines
}

/// True when `s` is a well-formed ISO `YYYY-MM-DD` date:
/// exactly ten chars, `dddd-dd-dd`, with month `01..=12` and
/// day `01..=31`. A shallow calendar check -- it does not
/// reject Feb 30 -- which is enough for lexical watermark
/// comparison (ISO dates sort lexically iff well-formed).
pub fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |r: &[u8]| r.iter().all(u8::is_ascii_digit);
    if !digits(&b[0..4]) || !digits(&b[5..7]) || !digits(&b[8..10]) {
        return false;
    }
    let two = |a: u8, c: u8| (a - b'0') * 10 + (c - b'0');
    let month = two(b[5], b[6]);
    let day = two(b[8], b[9]);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// Canonical markdown column width for the project (matches the
/// 80-column rule in `rustfmt.toml`). Shared by the `changelog`
/// and `todo` bullet writers.
pub const MARKDOWN_WIDTH: usize = 80;

/// Split file content into owned lines. Drops the trailing
/// newline; pair with [`rejoin`] to restore it. Shared by the
/// `changelog` and `todo` line-editing commands.
#[must_use]
pub fn to_owned_lines(content: &str) -> Vec<String> {
    content.lines().map(str::to_owned).collect()
}

/// Rejoin lines with `\n`, restoring a trailing newline when the
/// original file had one (so a rewrite stays byte-clean).
#[must_use]
pub fn rejoin(lines: &[String], ends_with_newline: bool) -> String {
    let mut out = lines.join("\n");
    if ends_with_newline {
        out.push('\n');
    }
    out
}

/// Locate a `## <heading>` section. Returns
/// `(heading_index, section_end)` where `section_end` is the
/// index of the next `## ` heading (or the line count) -- so the
/// section body is `(heading_index + 1)..section_end`. `None`
/// when the heading is absent. Shared by the `changelog` and
/// `todo` commands.
#[must_use]
pub fn section_bounds(
    lines: &[String],
    heading: &str,
) -> Option<(usize, usize)> {
    let h = lines.iter().position(|l| l.trim() == heading)?;
    let end = ((h + 1)..lines.len())
        .find(|&i| lines[i].starts_with("## "))
        .unwrap_or(lines.len());
    Some((h, end))
}

/// Reject empty / whitespace-only user input for the mechanical
/// markdown writers, so a contentless `- ` (or `- **slug** -- `)
/// stub is never written into `CHANGELOG.md` / `docs/todo.md`.
/// `label` names the offending field in the error.
///
/// # Errors
///
/// Returns an error when `value` is empty or only whitespace.
pub fn require_nonempty(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(())
    }
}

/// Word-wrap `text` into markdown bullet lines that stay within
/// `width` bytes (equal to display columns for the ASCII-leaning
/// markdown this project writes; a multibyte char counts as its
/// UTF-8 byte length, so a line never *exceeds* `width` columns
/// but may wrap a little early). The first line is prefixed with
/// `first` (e.g. `"- "`), continuation lines with `cont` (e.g.
/// `"  "`), matching the project's 80-column markdown + 2-space
/// hanging indent for list items. A single word longer than the
/// available width overflows onto its own line rather than being
/// split mid-word. Shared by the `changelog` and `todo` writers.
#[must_use]
pub fn wrap_markdown(
    text: &str,
    first: &str,
    cont: &str,
    width: usize,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::from(first);
    let mut has_word = false;
    for word in text.split_whitespace() {
        let would_be = cur.len() + 1 + word.len();
        if has_word && would_be > width {
            lines.push(cur);
            cur = String::from(cont);
            cur.push_str(word);
        } else {
            if has_word {
                cur.push(' ');
            }
            cur.push_str(word);
        }
        has_word = true;
    }
    lines.push(cur);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stderr_tail_returns_last_n_non_empty() {
        let stderr = "one\n\ntwo\nthree\n\nfour\n";
        let tail = stderr_tail(stderr, 2);
        assert_eq!(tail, vec!["three", "four"]);
    }

    #[test]
    fn stderr_tail_handles_short_input() {
        let stderr = "only-one";
        let tail = stderr_tail(stderr, 5);
        assert_eq!(tail, vec!["only-one"]);
    }

    #[test]
    fn is_iso_date_accepts_valid_and_rejects_junk() {
        assert!(is_iso_date("2026-07-16"));
        assert!(is_iso_date("1999-12-31"));
        assert!(!is_iso_date("2026-13-01")); // month 13
        assert!(!is_iso_date("2026-07-32")); // day 32
        assert!(!is_iso_date("2026-7-16")); // not zero-padded
        assert!(!is_iso_date("2026/07/16")); // wrong separator
        assert!(!is_iso_date("not a date"));
        assert!(!is_iso_date("2026-07-16 "));
    }

    #[test]
    fn pair_with_locations_keeps_kept_lines_and_following_arrows() {
        let input = "\
keep me
    --> a/b.rs:1:1
drop me
    --> should/not/appear.rs:2:2
keep me too";
        let out = pair_with_locations(input, |l| l.starts_with("keep"));
        assert_eq!(out.len(), 3);
        assert!(out[0].starts_with("keep me"));
        assert!(out[1].contains("--> a/b.rs:1:1"));
        assert!(out[2].starts_with("keep me too"));
        // The `-->` after the dropped line must not appear.
        assert!(!out.iter().any(|l| l.contains("should/not/appear")));
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
    fn wrap_markdown_short_text_is_one_line() {
        assert_eq!(
            wrap_markdown("hello world", "- ", "  ", 80),
            vec!["- hello world"],
        );
    }

    #[test]
    fn wrap_markdown_wraps_at_width_with_hanging_indent() {
        // Width 20 forces a wrap; continuation uses the 2-space
        // prefix, and no line exceeds the width.
        let lines = wrap_markdown("alpha beta gamma delta", "- ", "  ", 20);
        assert_eq!(lines, vec!["- alpha beta gamma", "  delta"],);
        assert!(lines.iter().all(|l| l.len() <= 20));
    }

    #[test]
    fn wrap_markdown_overflows_a_single_long_word() {
        let lines =
            wrap_markdown("short superlongwordthatoverflows", "- ", "  ", 10);
        assert_eq!(lines, vec!["- short", "  superlongwordthatoverflows"]);
    }

    #[test]
    fn section_bounds_finds_body_range() {
        let lines = to_owned_lines("## A\n\n- one\n\n## B\n\n- two\n");
        assert_eq!(section_bounds(&lines, "## A"), Some((0, 4)));
        assert_eq!(section_bounds(&lines, "## B"), Some((4, lines.len())));
        assert_eq!(section_bounds(&lines, "## Missing"), None);
    }

    #[test]
    fn rejoin_round_trips_trailing_newline() {
        let with = "a\nb\n";
        assert_eq!(rejoin(&to_owned_lines(with), true), with);
        let without = "a\nb";
        assert_eq!(rejoin(&to_owned_lines(without), false), without);
    }

    #[test]
    fn require_nonempty_accepts_content_rejects_blank() {
        assert!(require_nonempty("x", "hello").is_ok());
        let err = require_nonempty("--summary", "   ").unwrap_err();
        assert!(err.contains("--summary"));
        assert!(require_nonempty("x", "").is_err());
    }
}
