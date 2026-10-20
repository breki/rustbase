//! Small text helpers the gates share with a project's own xtask
//! commands: filtering cargo's diagnostics and recognising ISO
//! dates.

/// Every line `keep` accepts, plus the `-->` location line
/// immediately after a kept line, so each diagnostic arrives with
/// its `file:line`. A `-->` after a dropped line is never kept.
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

/// The last `n` non-empty lines of any captured output (stderr, or
/// stdout when stderr is empty).
pub fn stderr_tail(stderr: &str, n: usize) -> Vec<&str> {
    let mut lines: Vec<&str> =
        stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(n);
    lines.drain(..start);
    lines
}

/// True for a real cargo/rustc diagnostic line (`warning:`,
/// `error[`, `error:`), false for the summary lines that share
/// those prefixes: `could not compile` / `could not document` (one
/// per failing target), `aborting due to`, `build failed`, the
/// `... generated N warnings` counts and `N warning(s) emitted`. A summary repeats what the
/// diagnostics already say and pushes them out of a short report.
pub fn is_cargo_diagnostic(line: &str) -> bool {
    // `") generated "` is anchored to its grammar (`crate (target)
    // generated N warnings`), so a lint message merely containing
    // the word "generated" is kept.
    let is_summary = line.contains("could not compile")
        || line.contains("could not document")
        || line.contains("aborting due to")
        || line.contains("build failed")
        || line.contains(") generated ")
        || line.contains(" warning emitted")
        || line.contains(" warnings emitted");
    !is_summary
        && (line.starts_with("warning:")
            || line.starts_with("error[")
            || line.starts_with("error:"))
}

/// True when `s` is a well-formed ISO `YYYY-MM-DD` date:
/// exactly ten chars, `dddd-dd-dd`, with month `01..=12` and
/// day `01..=31`. A shape check, not a calendar: it accepts
/// Feb 30. Well-formed ISO dates sort correctly as plain text.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stderr_tail_returns_the_last_n_non_empty_lines() {
        assert_eq!(
            stderr_tail("one\n\ntwo\nthree\n\nfour\n", 2),
            ["three", "four"]
        );
        assert_eq!(stderr_tail("only-one", 5), ["only-one"]);
    }

    #[test]
    fn cargo_diagnostics_are_kept_and_their_summaries_dropped() {
        for kept in [
            "warning: unresolved link to `Foo`",
            "error[E0433]: failed to resolve",
            "error: this lint is denied",
            "warning: a message that mentions generated code",
        ] {
            assert!(is_cargo_diagnostic(kept), "{kept}");
        }
        for dropped in [
            "error: could not compile `rustbase` (lib test) due to 1 error",
            "error: could not document `rustbase`",
            "error: aborting due to 2 previous errors",
            "warning: build failed, waiting for other jobs to finish...",
            "warning: `xtask` (bin \"xtask\") generated 1 warning",
            "warning: 1 warning emitted",
            "   --> src/lib.rs:1:1",
        ] {
            assert!(!is_cargo_diagnostic(dropped), "{dropped}");
        }
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
}
