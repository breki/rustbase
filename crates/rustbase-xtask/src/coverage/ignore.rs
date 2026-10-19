//! The `--ignore-filename-regex` the coverage gate passes to
//! llvm-cov: a built-in baseline plus the project's patterns from
//! `[workspace.metadata.coverage] ignore` in the root `Cargo.toml`.

use std::fs;
use std::path::Path;

/// Regex passed to `llvm-cov --ignore-filename-regex`, on both
/// path separators: the `src/main.rs` bootstrap and everything
/// under `src/bin/`. A binary's shell is exercised by spawning it,
/// which llvm-cov cannot fully credit; the testable logic belongs
/// in a library crate.
pub(super) const IGNORE_REGEX: &str = r"src[/\\](main\.rs$|bin[/\\])";

/// The full `--ignore-filename-regex`: [`IGNORE_REGEX`] merged with
/// the project's `[workspace.metadata.coverage] ignore` patterns.
/// An unreadable manifest is an error rather than the baseline:
/// the baseline would drop the project's exclusions and report
/// the excluded files as a coverage regression.
pub(super) fn ignore_regex(root: &Path) -> Result<String, String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_repo;

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
}
