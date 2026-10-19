//! Small text helpers the gates share with a project's own xtask
//! commands: filtering cargo's diagnostics and recognising ISO
//! dates.

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
