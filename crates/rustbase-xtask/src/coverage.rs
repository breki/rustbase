//! The line-coverage gate.
//!
//! Runs `cargo llvm-cov` over the workspace (the tooling crate
//! left out) and fails when the workspace falls below
//! [`Thresholds::coverage_overall`](crate::Thresholds) or any one
//! file below [`Thresholds::coverage_file`](crate::Thresholds). A
//! failure names the files to look at and their uncovered lines.
//!
//! A project excludes hardware-bound files with regex fragments in
//! `[workspace.metadata.coverage] ignore` in the root `Cargo.toml`.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::process::{self, Captured};
use crate::{Config, TOOLING_CRATE, Thresholds};

/// Regex passed to `llvm-cov --ignore-filename-regex`, on both
/// path separators: the `src/main.rs` bootstrap and everything
/// under `src/bin/`. A binary's shell is exercised by spawning it,
/// which llvm-cov cannot fully credit; the testable logic belongs
/// in a library crate.
const IGNORE_REGEX: &str = r"src[/\\](main\.rs$|bin[/\\])";

/// How many files an overall failure lists.
const LEAST_COVERED_SHOWN: usize = 8;

/// What a passing run measured.
#[derive(Debug, PartialEq)]
pub struct Report {
    /// Covered lines.
    pub covered: u64,
    /// Counted lines.
    pub total: u64,
    /// Line coverage of the workspace, in percent.
    pub line_pct: f64,
}

/// Runs the gate in the workspace root.
///
/// # Errors
///
/// When coverage is below a threshold (naming the files and their
/// uncovered lines), when the run measured nothing, or when the
/// root `Cargo.toml` or the tool cannot be read.
pub fn check(cfg: &Config) -> Result<Report, String> {
    measure(cfg, |args| process::cargo(&cfg.root, args))
}

/// [`check`] with the cargo spawn injected.
fn measure(
    cfg: &Config,
    mut cargo: impl FnMut(&[&str]) -> Result<Captured, String>,
) -> Result<Report, String> {
    let ignore = ignore_regex(&cfg.root)?;
    // Stale `.profraw` files from earlier runs inflate the line
    // denominator: llvm-cov merges every profile it finds, so a
    // real 98% can read as 70%. Clean first.
    let clean = cargo(&["llvm-cov", "clean", "--workspace"])?;
    if !clean.ok {
        return Err(format!("cargo llvm-cov clean failed:\n{}", clean.stderr));
    }
    // Without `--summary-only`, so the per-file `segments` array is
    // there to compute uncovered line ranges from.
    let out = cargo(&[
        "llvm-cov",
        "--workspace",
        "--exclude",
        TOOLING_CRATE,
        "--ignore-filename-regex",
        &ignore,
        "--json",
    ])?;
    if !out.ok {
        return Err(format!("cargo llvm-cov failed:\n{}", out.stderr));
    }
    let json: Value = serde_json::from_str(&out.stdout)
        .map_err(|e| format!("failed to parse coverage JSON: {e}"))?;
    evaluate(&json, &cfg.thresholds)
}

/// The full `--ignore-filename-regex`: [`IGNORE_REGEX`] merged with
/// the project's `[workspace.metadata.coverage] ignore` patterns.
/// An unreadable manifest is an error rather than the baseline:
/// the baseline would drop the project's exclusions and report
/// the excluded files as a coverage regression.
fn ignore_regex(root: &Path) -> Result<String, String> {
    let path = root.join("Cargo.toml");
    let manifest = fs::read_to_string(&path).map_err(|e| {
        format!(
            "cannot read {} for [workspace.metadata.coverage]: {e}",
            path.display()
        )
    })?;
    let patterns = validate_ignore_patterns(parse_coverage_ignore(&manifest))?;
    Ok(build_ignore_regex(IGNORE_REGEX, &patterns))
}

/// The baseline followed by the project's patterns, joined by `|`
/// (top-level alternation).
fn build_ignore_regex(default: &str, user: &[String]) -> String {
    let mut out = String::from(default);
    for pattern in user {
        out.push('|');
        out.push_str(pattern);
    }
    out
}

/// Drop blank patterns and reject any that match every file. A
/// blank would append a bare `|` (matching every path); `.`, `.*`,
/// `.+` match everything outright. A blank is dropped as a typo, a
/// match-all fails loudly.
fn validate_ignore_patterns(
    patterns: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut kept = Vec::new();
    for pattern in patterns {
        let trimmed = pattern.trim();
        if trimmed.is_empty() {
            continue;
        }
        if matches!(trimmed, "." | ".*" | ".+" | ".*?") {
            return Err(format!(
                "coverage ignore pattern {pattern:?} matches every file \
                 -- refusing a vacuous coverage gate; fix or remove it in \
                 [workspace.metadata.coverage] ignore"
            ));
        }
        kept.push(pattern);
    }
    Ok(kept)
}

/// The `[workspace.metadata.coverage]` `ignore = [...]` patterns
/// of a root `Cargo.toml`, in declared order; empty when the
/// section or key is absent. A dependency-free scan (no `toml`
/// crate), quote-aware so `]` and `#` inside a pattern are
/// literal; both `"..."` and `'...'` strings are read. Only a
/// line-leading `ignore = [...]` under that header is recognised,
/// not the dotted-key or inline-table spellings.
fn parse_coverage_ignore(manifest: &str) -> Vec<String> {
    let body = coverage_section_body(manifest);
    ignore_value_region(&body)
        .map(scan_quoted)
        .unwrap_or_default()
}

/// The lines of the `[workspace.metadata.coverage]` section, up to
/// the next section header, kept verbatim so the value scan sees
/// comments and brackets in context.
fn coverage_section_body(manifest: &str) -> String {
    let mut in_section = false;
    let mut body = String::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let name = trimmed.trim_matches(|c| c == '[' || c == ']').trim();
            in_section = name == "workspace.metadata.coverage";
            continue;
        }
        if in_section {
            body.push_str(line);
            body.push('\n');
        }
    }
    body
}

/// The text after the `=` of a line-leading `ignore` key, through
/// the end of `body` so a multi-line array is scanned whole.
fn ignore_value_region(body: &str) -> Option<&str> {
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        if let Some(after) = line.trim_start().strip_prefix("ignore")
            && after.trim_start().starts_with('=')
            && let Some(eq) = line.find('=')
        {
            return Some(&body[offset + eq + 1..]);
        }
        offset += line.len();
    }
    None
}

/// The contents of each quoted string in `region`, stopping at the
/// first `]` outside a quote (the array's close). `#` outside a
/// quote starts a comment to end of line. An unterminated final
/// quote is discarded.
fn scan_quoted(region: &str) -> Vec<String> {
    enum State {
        Normal,
        Comment,
        Double,
        Single,
    }
    let mut state = State::Normal;
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in region.chars() {
        match state {
            State::Normal => match c {
                '#' => state = State::Comment,
                '"' => state = State::Double,
                '\'' => state = State::Single,
                ']' => break,
                _ => {}
            },
            State::Comment => {
                if c == '\n' {
                    state = State::Normal;
                }
            }
            State::Double | State::Single => {
                let close = if matches!(state, State::Double) {
                    '"'
                } else {
                    '\''
                };
                if c == close {
                    out.push(std::mem::take(&mut cur));
                    state = State::Normal;
                } else {
                    cur.push(c);
                }
            }
        }
    }
    out
}

/// Why the gate failed. Kept apart from its rendering in
/// [`format_failure`].
enum CoverageFailure {
    /// The workspace fell below the overall threshold.
    /// `least_covered` names where to look, not only how much.
    Overall {
        pct: f64,
        threshold: f64,
        least_covered: Vec<FailingModule>,
    },
    /// Files fell below the per-file threshold while the overall
    /// figure held.
    Modules(Vec<FailingModule>),
}

/// A file below a threshold, with its uncovered line ranges.
struct FailingModule {
    /// The path after `src/`.
    name: String,
    pct: f64,
    /// Sorted, merged, inclusive on both ends.
    ranges: Vec<(u64, u64)>,
}

/// Judge an `llvm-cov --json` report against `t`.
fn evaluate(json: &Value, t: &Thresholds) -> Result<Report, String> {
    let totals = &json["data"][0]["totals"]["lines"];
    let line_pct = totals["percent"].as_f64().ok_or("missing lines.percent")?;
    let covered = totals["covered"].as_u64().ok_or("missing lines.covered")?;
    let total = totals["count"].as_u64().ok_or("missing lines.count")?;
    let files = &json["data"][0]["files"];

    // `validate_ignore_patterns` catches only the match-all
    // spellings it knows; whatever excluded everything, a run that
    // measured nothing must not pass.
    let measured = files.as_array().map_or(0, Vec::len);
    if measured == 0 || total == 0 {
        return Err(format!(
            "coverage measured {measured} files ({total} lines) -- the gate \
             is vacuous. Check [workspace.metadata.coverage] ignore in the \
             root Cargo.toml; a pattern there is excluding everything."
        ));
    }

    let failure = if line_pct < t.coverage_overall {
        Some(CoverageFailure::Overall {
            pct: line_pct,
            threshold: t.coverage_overall,
            least_covered: least_covered(files, LEAST_COVERED_SHOWN)?,
        })
    } else {
        let below = scan_modules(files, t.coverage_file)?;
        (!below.is_empty()).then_some(CoverageFailure::Modules(below))
    };
    match failure {
        Some(f) => Err(format_failure(&f)),
        None => Ok(Report {
            covered,
            total,
            line_pct,
        }),
    }
}

fn format_failure(failure: &CoverageFailure) -> String {
    match failure {
        CoverageFailure::Overall {
            pct,
            threshold,
            least_covered,
        } => {
            let mut msg =
                format!("coverage {pct:.1}% is below {threshold}% threshold");
            if !least_covered.is_empty() {
                msg.push_str("; least covered:");
                write_modules(&mut msg, least_covered);
            }
            msg
        }
        CoverageFailure::Modules(modules) => {
            let mut msg = String::from("modules below coverage threshold:");
            write_modules(&mut msg, modules);
            msg
        }
    }
}

/// Files below full coverage, least covered first, at most `limit`.
fn least_covered(
    files: &Value,
    limit: usize,
) -> Result<Vec<FailingModule>, String> {
    let mut all = scan_modules(files, 100.0)?;
    all.sort_by(|a, b| a.pct.total_cmp(&b.pct));
    all.truncate(limit);
    Ok(all)
}

/// Files below `threshold` with their uncovered line ranges. A file
/// with no counted lines gives no percentage and is skipped.
fn scan_modules(
    files: &Value,
    threshold: f64,
) -> Result<Vec<FailingModule>, String> {
    let Some(arr) = files.as_array() else {
        return Ok(Vec::new());
    };
    let mut below = Vec::new();
    for file in arr {
        let name = file["filename"].as_str().unwrap_or("?");
        let pct = file["summary"]["lines"]["percent"].as_f64().unwrap_or(0.0);
        let count = file["summary"]["lines"]["count"].as_u64().unwrap_or(0);
        if count == 0 || pct >= threshold {
            continue;
        }
        let segments = parse_segments(&file["segments"]).map_err(|e| {
            format!("failed to parse llvm-cov segments for {name}: {e}")
        })?;
        below.push(FailingModule {
            name: shorten_path(name).to_string(),
            pct,
            ranges: uncovered_ranges(&segments),
        });
    }
    Ok(below)
}

/// One `name: pct` line per module, each followed by its uncovered
/// ranges when it has any.
fn write_modules(msg: &mut String, modules: &[FailingModule]) {
    for m in modules {
        let _ = write!(msg, "\n    {}: {:.1}%", m.name, m.pct);
        if !m.ranges.is_empty() {
            let _ =
                write!(msg, "\n      uncovered: {}", format_ranges(&m.ranges));
        }
    }
}

/// The fields of an llvm-cov segment this gate reads. The wire form
/// is `[line, col, count, has_count, is_region_entry, is_gap]`.
#[derive(Debug)]
struct Segment {
    line: u64,
    count: u64,
    has_count: bool,
    is_gap: bool,
}

impl Segment {
    fn is_uncovered(&self) -> bool {
        self.has_count && self.count == 0 && !self.is_gap
    }
}

/// Parse a `segments` array; null or empty gives no segments. Any
/// segment not of the six-element shape is an error rather than a
/// misread (llvm-cov before LLVM 12 emitted five).
fn parse_segments(segments: &Value) -> Result<Vec<Segment>, String> {
    if segments.is_null() {
        return Ok(Vec::new());
    }
    let arr = segments
        .as_array()
        .ok_or("segment shape mismatch: not an array")?;
    arr.iter().map(parse_segment).collect()
}

fn parse_segment(v: &Value) -> Result<Segment, String> {
    let shape = || format!("segment shape mismatch: {v}");
    let a = v.as_array().filter(|a| a.len() == 6).ok_or_else(shape)?;
    let num = |i: usize| a[i].as_u64().ok_or_else(shape);
    let flag = |i: usize| a[i].as_bool().ok_or_else(shape);
    num(1)?;
    flag(4)?;
    Ok(Segment {
        line: num(0)?,
        count: num(2)?,
        has_count: flag(3)?,
        is_gap: flag(5)?,
    })
}

/// Uncovered line ranges: a segment's count applies until the next
/// segment starts. The last segment has no successor and is
/// reported as its own line, so a file ending in an uncovered span
/// still names where it starts.
fn uncovered_ranges(segments: &[Segment]) -> Vec<(u64, u64)> {
    let mut raw = Vec::new();
    for window in segments.windows(2) {
        let (seg, next) = (&window[0], &window[1]);
        if !seg.is_uncovered() {
            continue;
        }
        let end = if next.line > seg.line {
            next.line - 1
        } else {
            seg.line
        };
        raw.push((seg.line, end));
    }
    if let Some(last) = segments.last()
        && last.is_uncovered()
    {
        raw.push((last.line, last.line));
    }
    merge_ranges(raw)
}

/// Merge overlapping or adjacent inclusive ranges: `(5,7)` and
/// `(8,10)` become `(5,10)`.
fn merge_ranges(mut ranges: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    ranges.sort_unstable();
    let mut merged: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
    for (s, e) in ranges {
        match merged.last_mut() {
            Some(last) if s <= last.1.saturating_add(1) => {
                last.1 = last.1.max(e);
            }
            _ => merged.push((s, e)),
        }
    }
    merged
}

/// `(84, 93), (209, 209)` -> `"84-93, 209"`.
fn format_ranges(ranges: &[(u64, u64)]) -> String {
    let parts: Vec<String> = ranges
        .iter()
        .map(|(s, e)| {
            if s == e {
                format!("{s}")
            } else {
                format!("{s}-{e}")
            }
        })
        .collect();
    parts.join(", ")
}

/// The part of a path after `src/`.
fn shorten_path(name: &str) -> &str {
    name.rsplit_once("src\\")
        .or_else(|| name.rsplit_once("src/"))
        .map_or(name, |(_, rest)| rest)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;
    use crate::test_repo;

    fn file(name: &str, pct: f64, count: u64) -> Value {
        json!({"filename": name, "segments": [],
               "summary": {"lines": {"percent": pct, "count": count}}})
    }

    fn report(pct: f64, files: &[Value]) -> Value {
        json!({"data": [{"totals": {"lines":
            {"percent": pct, "covered": 9, "count": 10}},
            "files": files}]})
    }

    fn t() -> Thresholds {
        Thresholds::default()
    }

    #[test]
    fn build_ignore_regex_returns_default_without_user_patterns() {
        assert_eq!(build_ignore_regex(IGNORE_REGEX, &[]), IGNORE_REGEX);
    }

    #[test]
    fn build_ignore_regex_appends_user_patterns_in_order() {
        let user = vec!["src/audio/native.rs".to_string(), "src/x.rs".into()];
        assert_eq!(
            build_ignore_regex("base", &user),
            "base|src/audio/native.rs|src/x.rs"
        );
    }

    #[test]
    fn parse_coverage_ignore_reads_an_inline_array() {
        let toml = "[workspace.metadata.coverage]\n\
                    ignore = [\"src/a.rs\", \"src/b.rs\"]\n";
        assert_eq!(parse_coverage_ignore(toml), ["src/a.rs", "src/b.rs"]);
    }

    #[test]
    fn parse_coverage_ignore_reads_a_multiline_array() {
        let toml = "[workspace.metadata.coverage]\nignore = [\n  \
                    \"src/a.rs\",  # a comment\n  \"src/b.rs\",\n]\n";
        assert_eq!(parse_coverage_ignore(toml), ["src/a.rs", "src/b.rs"]);
    }

    #[test]
    fn parse_coverage_ignore_keeps_brackets_inside_a_pattern() {
        // Regex char classes contain `]`, which must not end the
        // array. The TOML value is `src[/\\]a\.rs`.
        let toml = "[workspace.metadata.coverage]\nignore = [\n  \
                    \"src[/\\\\]a\\.rs\",\n  \"src/b\\.rs\",\n]\n";
        assert_eq!(
            parse_coverage_ignore(toml),
            [r"src[/\\]a\.rs", r"src/b\.rs"]
        );
    }

    #[test]
    fn parse_coverage_ignore_keeps_hash_inside_a_pattern() {
        let toml = "[workspace.metadata.coverage]\n\
                    ignore = [\"src/v#2/a.rs\"]\n";
        assert_eq!(parse_coverage_ignore(toml), ["src/v#2/a.rs"]);
    }

    #[test]
    fn parse_coverage_ignore_reads_single_quoted_literals() {
        let toml = "[workspace.metadata.coverage]\n\
                    ignore = ['src[/\\]a.rs']\n";
        assert_eq!(parse_coverage_ignore(toml), [r"src[/\]a.rs"]);
    }

    #[test]
    fn parse_coverage_ignore_drops_an_unterminated_quote() {
        let toml = "[workspace.metadata.coverage]\nignore = [\"a.rs\", \"b";
        assert_eq!(parse_coverage_ignore(toml), ["a.rs"]);
    }

    #[test]
    fn parse_coverage_ignore_empty_when_section_absent() {
        let toml = "[workspace]\nmembers = []\n\n\
                    [profile.release-fast]\ninherits = \"release\"\n";
        let got = parse_coverage_ignore(toml);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn parse_coverage_ignore_empty_when_key_absent() {
        // A later section's `ignore` must not leak in.
        let toml = "[workspace.metadata.coverage]\nother = 1\n\n\
                    [other.section]\nignore = [\"nope.rs\"]\n";
        let got = parse_coverage_ignore(toml);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn validate_ignore_patterns_drops_blanks() {
        let got = validate_ignore_patterns(vec![
            "src/a.rs".into(),
            String::new(),
            "  ".into(),
        ])
        .unwrap();
        assert_eq!(got, ["src/a.rs"]);
    }

    #[test]
    fn validate_ignore_patterns_rejects_match_all() {
        for p in [".", ".*", ".+", ".*?"] {
            assert!(
                validate_ignore_patterns(vec![p.to_string()]).is_err(),
                "{p:?} should be rejected"
            );
        }
    }

    #[test]
    fn ignore_regex_reads_the_root_manifest() {
        let dir = test_repo::empty_dir();
        fs::write(
            dir.0.join("Cargo.toml"),
            "[workspace.metadata.coverage]\nignore = ['src/io\\.rs']\n",
        )
        .unwrap();
        assert_eq!(
            ignore_regex(&dir.0).unwrap(),
            format!(r"{IGNORE_REGEX}|src/io\.rs")
        );
    }

    #[test]
    fn an_unreadable_manifest_is_an_error_not_the_baseline() {
        // The baseline would drop the project's exclusions and
        // report them as a coverage regression.
        let dir = test_repo::empty_dir();
        let e = ignore_regex(&dir.0).unwrap_err();
        assert!(e.contains("cannot read"), "{e}");
        assert!(e.contains("Cargo.toml"), "{e}");
    }

    fn seg(line: u64, count: u64, is_gap: bool) -> Segment {
        Segment {
            line,
            count,
            has_count: true,
            is_gap,
        }
    }

    #[test]
    fn merge_ranges_handles_empty() {
        let got = merge_ranges(Vec::new());
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn merge_ranges_combines_adjacent_and_overlapping_unsorted() {
        let out = merge_ranges(vec![(20, 22), (5, 7), (8, 10), (9, 12)]);
        assert_eq!(out, vec![(5, 12), (20, 22)]);
    }

    #[test]
    fn format_ranges_collapses_single_line() {
        assert_eq!(format_ranges(&[(5, 5), (10, 12)]), "5, 10-12");
        assert_eq!(format_ranges(&[]), "");
    }

    #[test]
    fn uncovered_ranges_extracts_zero_count_spans() {
        let segs = vec![
            seg(10, 0, false),
            seg(20, 5, false),
            seg(30, 0, false),
            seg(40, 5, false),
        ];
        assert_eq!(uncovered_ranges(&segs), vec![(10, 19), (30, 39)]);
    }

    #[test]
    fn uncovered_ranges_includes_trailing_uncovered_segment() {
        // `windows(2)` never yields the last segment first.
        let segs = vec![seg(10, 5, false), seg(20, 0, false)];
        assert_eq!(uncovered_ranges(&segs), vec![(20, 20)]);
    }

    #[test]
    fn uncovered_ranges_on_one_line_ends_on_that_line() {
        let segs = vec![seg(7, 0, false), seg(7, 3, false)];
        assert_eq!(uncovered_ranges(&segs), vec![(7, 7)]);
    }

    #[test]
    fn uncovered_ranges_ignores_gap_regions() {
        let segs = vec![seg(10, 0, true), seg(20, 0, false)];
        assert_eq!(uncovered_ranges(&segs), vec![(20, 20)]);
    }

    #[test]
    fn parse_segments_accepts_six_element_arrays() {
        let segs =
            parse_segments(&json!([[10, 1, 0, true, true, false]])).unwrap();
        assert_eq!(segs.len(), 1);
        assert!(segs[0].is_uncovered());
    }

    #[test]
    fn parse_segments_rejects_wrong_shapes() {
        for bad in [
            json!([[10, 1, 0, true, true]]),
            json!([[10, "x", 0, true, true, false]]),
            json!({"not": "an array"}),
        ] {
            let e = parse_segments(&bad).unwrap_err();
            assert!(e.contains("shape mismatch"), "{e}");
        }
    }

    #[test]
    fn parse_segments_empty_or_null() {
        assert!(parse_segments(&Value::Null).unwrap().is_empty());
        assert!(parse_segments(&json!([])).unwrap().is_empty());
    }

    #[test]
    fn a_healthy_report_passes() {
        let r = evaluate(&report(95.0, &[file("/w/src/a.rs", 95.0, 10)]), &t());
        assert_eq!(
            r.unwrap(),
            Report {
                covered: 9,
                total: 10,
                line_pct: 95.0
            }
        );
    }

    #[test]
    fn a_run_that_measured_nothing_fails() {
        // A pattern spelling `validate_ignore_patterns` does not know
        // can still exclude every file.
        let e = evaluate(&report(100.0, &[]), &t()).unwrap_err();
        assert!(e.contains("vacuous"), "{e}");
    }

    #[test]
    fn overall_failure_names_the_least_covered_files() {
        let files = [
            file("/w/src/full.rs", 100.0, 10),
            file("/w/src/half.rs", 50.0, 10),
            file("/w/src/most.rs", 80.0, 10),
        ];
        let e = evaluate(&report(79.8, &files), &t()).unwrap_err();
        assert!(
            e.starts_with("coverage 79.8% is below 90% threshold"),
            "{e}"
        );
        let half = e.find("half.rs: 50.0%").expect(&e);
        let most = e.find("most.rs: 80.0%").expect(&e);
        assert!(half < most, "worst first: {e}");
        assert!(!e.contains("full.rs"), "{e}");
    }

    #[test]
    fn a_file_below_its_floor_fails_while_the_total_holds() {
        let mut low = file("/w/src/api/routes.rs", 72.5, 10);
        low["segments"] = json!([
            [84, 1, 0, true, true, false],
            [94, 1, 3, true, false, false]
        ]);
        let files = [file("/w/src/a.rs", 99.0, 100), low];
        let e = evaluate(&report(95.0, &files), &t()).unwrap_err();
        assert!(e.contains("api/routes.rs: 72.5%"), "{e}");
        assert!(e.contains("uncovered: 84-93"), "{e}");
    }

    #[test]
    fn thresholds_come_from_config() {
        let files = [file("/w/src/a.rs", 80.0, 10)];
        let loose = Thresholds {
            coverage_overall: 75.0,
            coverage_file: 70.0,
            ..t()
        };
        assert!(evaluate(&report(80.0, &files), &loose).is_ok());
        assert!(evaluate(&report(80.0, &files), &t()).is_err());
    }

    #[test]
    fn a_report_missing_its_totals_is_an_error() {
        assert!(evaluate(&json!({}), &t()).is_err());
    }

    #[test]
    fn bad_segments_on_a_failing_file_are_an_error() {
        let mut low = file("/w/src/a.rs", 10.0, 10);
        low["segments"] = json!([[1, 2]]);
        let e = evaluate(&report(95.0, &[low]), &t()).unwrap_err();
        assert!(e.contains("a.rs"), "{e}");
    }

    fn ok(stdout: &str) -> Captured {
        Captured {
            ok: true,
            status: String::new(),
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    fn failed(stderr: &str) -> Captured {
        Captured {
            ok: false,
            status: String::new(),
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }

    fn workspace(manifest: &str) -> (test_repo::TempRepo, Config) {
        let dir = test_repo::empty_dir();
        fs::write(dir.0.join("Cargo.toml"), manifest).unwrap();
        let cfg = Config::for_project("p", PathBuf::from(&dir.0));
        (dir, cfg)
    }

    #[test]
    fn measure_cleans_then_runs_llvm_cov_with_the_ignores() {
        let (_dir, cfg) =
            workspace("[workspace.metadata.coverage]\nignore = ['x\\.rs']\n");
        let calls = RefCell::new(Vec::new());
        let body = report(95.0, &[file("/w/src/a.rs", 95.0, 10)]).to_string();
        let r = measure(&cfg, |args| {
            calls.borrow_mut().push(args.join(" "));
            Ok(ok(&body))
        });
        assert!(r.is_ok(), "{r:?}");
        let calls = calls.into_inner();
        assert_eq!(calls[0], "llvm-cov clean --workspace");
        assert!(calls[1].contains("--exclude xtask"), "{}", calls[1]);
        assert!(calls[1].contains(r"|x\.rs"), "{}", calls[1]);
    }

    #[test]
    fn measure_reports_a_failed_tool_run() {
        let (_dir, cfg) = workspace("");
        let e = measure(&cfg, |_| Ok(failed("no profiler"))).unwrap_err();
        assert!(e.contains("clean failed") && e.contains("no profiler"));

        let mut n = 0;
        let e = measure(&cfg, |_| {
            n += 1;
            Ok(if n == 1 { ok("") } else { failed("boom") })
        })
        .unwrap_err();
        assert!(e.contains("llvm-cov failed") && e.contains("boom"), "{e}");

        let e = measure(&cfg, |_| Ok(ok("not json"))).unwrap_err();
        assert!(e.contains("parse coverage JSON"), "{e}");
    }

    #[test]
    fn measure_stops_on_an_unreadable_manifest_before_running() {
        let dir = test_repo::empty_dir();
        let cfg = Config::for_project("p", PathBuf::from(&dir.0));
        let e = measure(&cfg, |_| panic!("must not run")).unwrap_err();
        assert!(e.contains("cannot read"), "{e}");
    }

    #[test]
    fn shorten_path_keeps_what_follows_src() {
        assert_eq!(shorten_path("/w/crates/a/src/x/y.rs"), "x/y.rs");
        assert_eq!(shorten_path(r"C:\w\src\y.rs"), "y.rs");
        assert_eq!(shorten_path("plain.rs"), "plain.rs");
    }
}
