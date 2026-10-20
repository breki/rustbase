//! The record files -- the work queue (`docs/todo.md`), the three
//! reviewer logs (the backlogs) and the template-feedback file -- and
//! the `records-check` gate over them.
//!
//! A record file holds a flat list of entries. Each opens with a
//! column-0 `### <id>` heading (optionally `### <id> -- <title>`),
//! carries a **field block** of `**Label:** value` lines directly
//! beneath it, and then a prose body. The tooling reads the fields;
//! the prose is for people:
//!
//! ```text
//! ### rt-2026-10-20-some-finding
//!
//! **Category:** correctness
//! **Depends on:** another-entry
//!
//! Prose body.
//! ```
//!
//! The gate checks four things and nothing about phrasing: an id
//! used twice across the files, a field label outside
//! [`KNOWN_LABELS`], a heading id that is not kebab-case from a letter, and a
//! `Depends on` / `Supersedes` id that names no entry. An id in
//! prose is not checked: that is where a citation of a since-removed
//! entry lives.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::Config;
use crate::markdown::is_fence;

/// The record files a project gets by default, relative to its
/// root.
pub const DEFAULT_FILES: &[&str] = &[
    "docs/todo.md",
    "docs/developer/redteam-log.md",
    "docs/developer/artisan-log.md",
    "docs/developer/fresh-reader-log.md",
    "docs/developer/template-feedback.md",
];

/// The field labels an entry may carry: `Category` (a finding's
/// kind), `Summary` (a queue item's one line), `Source` (where the
/// entry came from, e.g. a downstream project), `Issue` (the GitHub
/// issue, as `#21`), and `Depends on` / `Supersedes` (other entries'
/// ids). Any other is a typo or a body line read as a field, and
/// either way the set should not grow unnoticed.
pub const KNOWN_LABELS: &[&str] = &[
    "Category",
    "Summary",
    "Source",
    "Issue",
    "Depends on",
    "Supersedes",
];

/// The fields whose values are entry ids the gate resolves.
const CROSS_REF_LABELS: &[&str] = &["Depends on", "Supersedes"];

/// One parsed entry.
#[derive(Debug)]
pub(crate) struct Entry {
    /// The id: the heading up to a ` -- `, or all of it.
    pub(crate) id: String,
    /// Whether the heading carries a ` -- <title>` after the id.
    pub(crate) titled: bool,
    /// Index of the `### ` heading in `content.lines()`.
    pub(crate) idx: usize,
    /// The field block, in file order.
    fields: Vec<Field>,
}

impl Entry {
    /// The value of the first field with this label.
    pub(crate) fn field(&self, label: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.label == label)
            .map(|f| f.value.as_str())
    }
}

/// One `**Label:** value` line of a field block.
#[derive(Debug)]
struct Field {
    label: String,
    value: String,
    /// Index of the line in `content.lines()`.
    idx: usize,
}

/// One thing the gate found wrong.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    file: String,
    /// 1-based line number.
    line: usize,
    /// Which check fired, e.g. `duplicate-id`.
    kind: &'static str,
    message: String,
}

/// True for a kebab-case id: lowercase letters and digits in
/// dash-separated runs, starting with a letter. The letter is what
/// refuses a bare date, the heading of a legacy feedback entry.
pub(crate) fn is_slug(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.split('-').all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// Line `idx`, `line`, as a `**Label:** value` field, or `None`. The
/// label may hold spaces but not `*`, so a bold run closing before
/// the colon is prose, not a field.
fn parse_field(line: &str, idx: usize) -> Option<Field> {
    let (label, after) = line.strip_prefix("**")?.split_once(":**")?;
    if label.contains('*') {
        return None;
    }
    let value = after.strip_prefix(' ').unwrap_or(after).trim();
    Some(Field {
        label: label.trim().to_owned(),
        value: value.to_owned(),
        idx,
    })
}

/// The entries of a record file: one per column-0 `### ` heading
/// outside a code block. The field block is the run of field lines
/// directly under the heading (after an optional blank line), ending
/// at the first line that is not one.
pub(crate) fn parse(content: &str) -> Vec<Entry> {
    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate() {
        if is_fence(line) {
            in_fence = !in_fence;
        }
        let Some(heading) = line.strip_prefix("### ").filter(|_| !in_fence)
        else {
            continue;
        };
        let (id, titled) = match heading.split_once(" -- ") {
            Some((id, _)) => (id.trim(), true),
            None => (heading.trim(), false),
        };
        let mut j = i + 1;
        if lines.get(j).is_some_and(|l| l.trim().is_empty()) {
            j += 1;
        }
        let mut fields = Vec::new();
        while let Some(field) = lines.get(j).and_then(|l| parse_field(l, j)) {
            fields.push(field);
            j += 1;
        }
        out.push(Entry {
            id: id.to_owned(),
            titled,
            idx: i,
            fields,
        });
    }
    out
}

/// The ids a cross-reference names: comma-separated, with a `-` or
/// em-dash placeholder for "none" dropped.
fn ref_ids(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "-" && *t != "\u{2014}")
}

/// Every check over the parsed `(file, entries)` pairs, sorted.
fn findings_for(parsed: &[(&str, Vec<Entry>)]) -> Vec<Finding> {
    // Where each id is declared, in file order.
    let mut sites: BTreeMap<&str, Vec<(&str, usize)>> = BTreeMap::new();
    for (name, entries) in parsed {
        for e in entries {
            sites.entry(&e.id).or_default().push((name, e.idx));
        }
    }
    let mut out = Vec::new();
    let mut push = |file: &str, idx: usize, kind, message| {
        out.push(Finding {
            file: file.to_owned(),
            line: idx + 1,
            kind,
            message,
        });
    };
    for (name, entries) in parsed {
        for e in entries {
            if !is_slug(&e.id) {
                let msg = format!(
                    "heading id `{}` is not kebab-case starting with a letter",
                    e.id
                );
                push(name, e.idx, "malformed-id", msg);
            }
            for f in &e.fields {
                if !KNOWN_LABELS.contains(&f.label.as_str()) {
                    let msg = format!("unknown field label `{}`", f.label);
                    push(name, f.idx, "unknown-label", msg);
                }
                if !CROSS_REF_LABELS.contains(&f.label.as_str()) {
                    continue;
                }
                for id in ref_ids(&f.value).filter(|id| !sites.contains_key(id))
                {
                    let msg =
                        format!("{} names `{id}`, which is no entry", f.label);
                    push(name, f.idx, "dangling-ref", msg);
                }
            }
        }
    }
    for (id, declared) in &sites {
        for (name, idx) in declared.iter().skip(1) {
            let msg = format!("id `{id}` is already used");
            push(name, *idx, "duplicate-id", msg);
        }
    }
    out.sort();
    out
}

/// Checks `text`, an entry about to be written to a record file:
/// exactly one entry, a kebab-case id, no unclosed code block, known
/// field labels. Its cross
/// references are left to [`check`], which sees every entry.
///
/// # Errors
///
/// What would make the records check reject the entry.
pub fn check_entry(text: &str) -> Result<(), String> {
    let entries = parse(text);
    let [entry] = entries.as_slice() else {
        return Err(format!(
            "an entry must be one entry, not {}: a body line starting \
             `### ` would open another",
            entries.len()
        ));
    };
    if !is_slug(&entry.id) {
        return Err(format!(
            "heading id `{}` is not kebab-case starting with a letter",
            entry.id
        ));
    }
    // A fence left open would hide every later entry in the file.
    if text.lines().filter(|l| is_fence(l)).count() % 2 == 1 {
        return Err("the entry ends inside an unclosed code block; close \
                    it with a fence line"
            .into());
    }
    if let Some(f) = entry
        .fields
        .iter()
        .find(|f| !KNOWN_LABELS.contains(&f.label.as_str()))
    {
        return Err(format!(
            "unknown field label `{}`: the first body line reads as a \
             field; start it with plain text, or use one of {}",
            f.label,
            KNOWN_LABELS.join(", ")
        ));
    }
    Ok(())
}

/// Runs the gate over `cfg.record_files`; on success, a one-line
/// summary (`5 files, 109 entries`) naming any file that does not
/// exist, which is skipped.
///
/// # Errors
///
/// Every finding, one per line, or a record file that exists but
/// cannot be read.
pub fn check(cfg: &Config) -> Result<String, String> {
    let mut parsed = Vec::new();
    let mut absent = Vec::new();
    for f in &cfg.record_files {
        match std::fs::read_to_string(cfg.root.join(f)) {
            Ok(text) => parsed.push((f.as_str(), parse(&text))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                absent.push(f.as_str());
            }
            Err(e) => return Err(format!("cannot read {f}: {e}")),
        }
    }
    let findings = findings_for(&parsed);
    if findings.is_empty() {
        let entries: usize = parsed.iter().map(|(_, e)| e.len()).sum();
        let mut detail = format!("{} files, {entries} entries", parsed.len());
        if !absent.is_empty() {
            let _ = write!(detail, "; absent: {}", absent.join(", "));
        }
        return Ok(detail);
    }
    let mut msg = format!("{} record finding(s):", findings.len());
    for f in &findings {
        let _ =
            write!(msg, "\n  {}:{} [{}] {}", f.file, f.line, f.kind, f.message);
    }
    Err(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_repo;

    #[test]
    fn a_heading_title_after_two_dashes_is_not_part_of_the_id() {
        let e = parse("### tf-2026-09-11-foo -- Foo the bar\n");
        assert_eq!(
            (e[0].id.as_str(), e[0].titled),
            ("tf-2026-09-11-foo", true)
        );
        let e = parse("### rt-2026-09-13-baz\n");
        assert_eq!(
            (e[0].id.as_str(), e[0].titled),
            ("rt-2026-09-13-baz", false)
        );
    }

    #[test]
    fn parse_field_reads_a_labelled_line() {
        let f = parse_field("**Depends on:** a-slug, b-slug", 7).unwrap();
        assert_eq!(
            (f.label.as_str(), f.value.as_str(), f.idx),
            ("Depends on", "a-slug, b-slug", 7)
        );
        // A bold run closing before the colon is prose, not a field,
        // and so is a colon outside the bold run.
        for prose in [
            "**Supersedes `tf-2026-08-10-old`.**",
            "**Bold `x`.**",
            "**Not a field**: x",
        ] {
            assert!(parse_field(prose, 0).is_none(), "{prose}");
        }
    }

    #[test]
    fn is_slug_takes_kebab_case_only() {
        assert!(is_slug("rt-2026-10-20-a-b"));
        for bad in [
            "",
            "RT-7",
            "a b",
            "a_b",
            "a--b",
            "-a",
            "a-",
            "2026-05-18",
            "2fa",
        ] {
            assert!(!is_slug(bad), "{bad}");
        }
    }

    #[test]
    fn the_field_block_ends_at_the_first_blank_line() {
        let src = "### rt-2026-09-13-thing\n\n**Category:** X\n\
                   **Depends on:** y\n\nBody.\n**Status:** prose\n";
        let e = parse(src);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].field("Category"), Some("X"));
        assert_eq!(e[0].field("Depends on"), Some("y"));
        assert_eq!(e[0].field("Status"), None);
        // Without the blank line under the heading too.
        assert_eq!(
            parse("### a\n**Category:** X\n")[0].field("Category"),
            Some("X")
        );
    }

    #[test]
    fn only_column_0_headings_outside_code_blocks_open_entries() {
        let src = "intro\n\n```\n### not-an-entry\n```\n\n  ### indented\n\
                   #### deeper\n### real\n";
        let e = parse(src);
        let ids: Vec<&str> = e.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["real"]);
        assert_eq!(e[0].idx, 8);
    }

    fn files<'a>(pairs: &[(&'a str, &str)]) -> Vec<(&'a str, Vec<Entry>)> {
        pairs.iter().map(|(n, t)| (*n, parse(t))).collect()
    }

    fn kinds(found: &[Finding]) -> Vec<&str> {
        found.iter().map(|f| f.kind).collect()
    }

    #[test]
    fn a_clean_set_has_no_findings() {
        let f = files(&[
            ("a.md", "### rt-2026-09-13-one\n\n**Category:** X\n"),
            ("b.md", "### two\n\n**Depends on:** rt-2026-09-13-one\n"),
        ]);
        assert_eq!(findings_for(&f), []);
    }

    #[test]
    fn a_dangling_reference_is_reported_in_either_field() {
        let f = files(&[(
            "a.md",
            "### one\n\n**Depends on:** two, gone\n\n\
             ### two\n\n**Supersedes:** \u{2014}\n\n\
             ### three\n\n**Supersedes:** -, lost\n",
        )]);
        let found = findings_for(&f);
        assert_eq!(kinds(&found), ["dangling-ref", "dangling-ref"]);
        assert!(found[0].message.contains("`gone`"), "{found:?}");
        assert_eq!((found[0].file.as_str(), found[0].line), ("a.md", 3));
        assert!(found[1].message.contains("`lost`"), "{found:?}");
    }

    #[test]
    fn a_duplicate_id_is_reported_at_each_later_site() {
        let f = files(&[
            ("a.md", "### one\n\n**Category:** X\n"),
            ("b.md", "### one -- again\n\n**Category:** Y\n\n### one\n"),
        ]);
        let found = findings_for(&f);
        assert_eq!(kinds(&found), ["duplicate-id", "duplicate-id"]);
        assert_eq!((found[0].file.as_str(), found[0].line), ("b.md", 1));
        assert_eq!(found[1].line, 5);
    }

    #[test]
    fn an_unknown_label_and_a_malformed_id_are_reported() {
        let f =
            files(&[("a.md", "### RT-7 the finding\n\n**Catgory:** typo\n")]);
        let found = findings_for(&f);
        assert_eq!(kinds(&found), ["malformed-id", "unknown-label"]);
        assert!(found[1].message.contains("`Catgory`"), "{found:?}");
    }

    #[test]
    fn an_id_in_prose_is_not_checked() {
        let f = files(&[(
            "a.md",
            "### one\n\n**Category:** X\n\nSupersedes tf-2026-01-01-gone.\n",
        )]);
        assert_eq!(findings_for(&f), []);
    }

    #[test]
    fn check_reads_the_configured_files() {
        let dir = test_repo::empty_dir();
        std::fs::write(dir.0.join("a.md"), "### one\n\n### two\n").unwrap();
        std::fs::write(dir.0.join("b.md"), "### one\n").unwrap();
        let mut cfg = Config::for_project("p", dir.0.clone());
        cfg.record_files = vec!["a.md".into()];
        assert_eq!(check(&cfg), Ok("1 files, 2 entries".into()));
        cfg.record_files.push("b.md".into());
        let e = check(&cfg).unwrap_err();
        assert!(e.starts_with("1 record finding(s):"), "{e}");
        assert!(e.contains("\n  b.md:1 [duplicate-id] id `one`"), "{e}");
    }

    #[test]
    fn check_entry_takes_one_well_formed_entry() {
        let ok = "### tf-2026-10-20-a -- A\n\n**Source:** bombyx\n\nBody.";
        assert_eq!(check_entry(ok), Ok(()));
        let e = check_entry("### tf-a -- A\n\n**Fix:** x").unwrap_err();
        assert!(e.contains("unknown field label `Fix`"), "{e}");
        let e = check_entry("### tf-a\n\ntext\n\n### tf-b\n").unwrap_err();
        assert!(e.contains("one entry, not 2"), "{e}");
        let e = check_entry("no heading").unwrap_err();
        assert!(e.contains("one entry, not 0"), "{e}");
        let e = check_entry("### Bad Id\n").unwrap_err();
        assert!(e.contains("not kebab-case"), "{e}");
        // An unclosed code block would hide every later entry in the
        // file from the check.
        let e = check_entry("### tf-a\n\nSee:\n```\nfoo").unwrap_err();
        assert!(e.contains("unclosed code block"), "{e}");
        assert_eq!(check_entry("### tf-a\n\n```\n### tf-x\n```"), Ok(()));
    }

    #[test]
    fn a_missing_record_file_is_skipped_and_named() {
        // A derived project may never have received one of the files.
        let dir = test_repo::empty_dir();
        std::fs::write(dir.0.join("a.md"), "### one\n").unwrap();
        let mut cfg = Config::for_project("p", dir.0.clone());
        cfg.record_files = vec!["a.md".into(), "gone.md".into()];
        assert_eq!(
            check(&cfg),
            Ok("1 files, 1 entries; absent: gone.md".into())
        );
        // A path that exists but cannot be read is still an error.
        std::fs::create_dir(dir.0.join("dir.md")).unwrap();
        cfg.record_files = vec!["dir.md".into()];
        assert!(check(&cfg).unwrap_err().contains("cannot read dir.md"));
    }

    #[test]
    fn a_project_gets_the_default_files() {
        let cfg = Config::for_project("p", "/w".into());
        assert_eq!(cfg.record_files, DEFAULT_FILES);
    }
}
