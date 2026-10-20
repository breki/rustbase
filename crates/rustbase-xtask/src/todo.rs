//! The work queue in `docs/todo.md`: list, add and complete items
//! without hand-editing the file.
//!
//! Each item is a headed entry -- an `### <slug>` heading at column
//! 0, a `**Summary:**` field line under it, then any prose body:
//!
//! ```text
//! ### host-network-isolation
//!
//! **Summary:** confirm the nftables rules survive a reboot
//!
//! Prose body, as long as it needs to be.
//! ```
//!
//! Other `**Label:** value` lines may follow the summary; they are
//! kept and ignored. The file is an introduction, then items only.
//! It holds live work only: completing an item removes it whole, and
//! what shipped is recorded by the commit and git history.

use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::Config;
use crate::markdown::{
    MARKDOWN_WIDTH, is_fence, rejoin_like, require_nonempty, to_owned_lines,
    wrap_markdown,
};

/// A queued item. Displays as `slug -- summary`, or the slug alone
/// when the summary is empty: the one-line form `list` prints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The kebab-case slug its heading carries.
    pub slug: String,
    /// The `**Summary:**` line, empty when the entry has none.
    pub summary: String,
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.summary.is_empty() {
            f.write_str(&self.slug)
        } else {
            write!(f, "{} -- {}", self.slug, self.summary)
        }
    }
}

/// The queued items, in file order.
///
/// # Errors
///
/// When `docs/todo.md` cannot be read, or has a shape `check_shape`
/// refuses (the old Pending/Done layout, or a non-item heading after
/// the first item).
pub fn list(cfg: &Config) -> Result<Vec<Item>, String> {
    let content = read(cfg)?;
    check_shape(&content)?;
    Ok(parse(&content)
        .into_iter()
        .map(|r| Item {
            summary: r.summary.unwrap_or_default(),
            slug: r.id,
        })
        .collect())
}

/// Appends a new item at the end of the queue.
///
/// # Errors
///
/// A slug that is not kebab-case or is already taken, a blank or
/// over-long summary (it must fit one 80-column `**Summary:**` line),
/// a body with a line that would read as a heading, or an unreadable
/// or unwritable `docs/todo.md`, or one whose shape `check_shape`
/// refuses.
pub fn add(
    cfg: &Config,
    slug: &str,
    summary: &str,
    body: Option<&str>,
) -> Result<(), String> {
    let slug = Slug::new(slug)?;
    require_nonempty("summary", summary)?;
    // Rendered before the file is read, so a bad argument fails the
    // same way with or without a readable `docs/todo.md`.
    let entry = entry_lines(slug.as_str(), summary, body)?;
    let content = read(cfg)?;
    check_shape(&content)?;
    if parse(&content).iter().any(|r| r.id == slug.as_str()) {
        return Err(format!(
            "slug '{}' already exists in docs/todo.md; pick another",
            slug.as_str()
        ));
    }
    write(cfg, &append_entry(&content, entry))
}

/// Removes a completed item, heading through body.
///
/// # Errors
///
/// A slug that is not kebab-case or names no item, or an unreadable
/// or unwritable `docs/todo.md`, or one whose shape `check_shape`
/// refuses.
pub fn done(cfg: &Config, slug: &str) -> Result<(), String> {
    let slug = Slug::new(slug)?;
    let content = read(cfg)?;
    check_shape(&content)?;
    write(cfg, &remove_entry(&content, &slug)?)
}

fn path(cfg: &Config) -> PathBuf {
    cfg.root.join("docs").join("todo.md")
}

fn read(cfg: &Config) -> Result<String, String> {
    let p = path(cfg);
    fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))
}

fn write(cfg: &Config, content: &str) -> Result<(), String> {
    let p = path(cfg);
    fs::write(&p, content).map_err(|e| format!("write {}: {e}", p.display()))
}

/// True for a kebab-case slug: the one shape an item heading has,
/// and the only one `remove_entry` can match.
fn is_slug(s: &str) -> bool {
    s.split('-').all(|seg| {
        !seg.is_empty()
            && seg
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    })
}

/// True for a markdown heading: one to six `#`, then a space or the
/// end of the line. `#7` and `#hashtag` are not headings.
fn reads_as_heading(line: &str) -> bool {
    let n = line.bytes().take_while(|&b| b == b'#').count();
    (1..=6).contains(&n) && matches!(line.as_bytes().get(n), None | Some(b' '))
}

/// One parsed entry.
struct Record {
    /// The slug its heading carries.
    id: String,
    /// Index of the `### ` heading in `content.lines()`.
    idx: usize,
    /// The `**Summary:**` field value, if the entry has one.
    summary: Option<String>,
}

/// The entries of a queue file: each column-0 `### <slug>` heading
/// with a `**Summary:**` field (any other `###` heading is prose).
/// The field block is the run of `**Label:** value` lines directly
/// under the heading (after an optional blank line), ending at the
/// first blank or non-field line.
fn parse(content: &str) -> Vec<Record> {
    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(id) = line.strip_prefix("### ").map(str::trim) else {
            continue;
        };
        if !is_slug(id) {
            continue;
        }
        let mut record = Record {
            id: id.to_owned(),
            idx: i,
            summary: None,
        };
        let mut j = i + 1;
        if lines.get(j).is_some_and(|l| l.trim().is_empty()) {
            j += 1;
        }
        while let Some((label, value)) =
            lines.get(j).and_then(|l| parse_field(l))
        {
            if label == "Summary" && record.summary.is_none() {
                record.summary = Some(value);
            }
            j += 1;
        }
        // A heading without a Summary is prose (a `### notes`
        // subsection), not an item.
        if record.summary.is_some() {
            out.push(record);
        }
    }
    out
}

/// One line as a `**Label:** value` field, or `None`. The label may
/// hold spaces but not `*`, so a bold run closing before the colon is
/// prose, not a field.
fn parse_field(line: &str) -> Option<(String, String)> {
    let (label, after) = line.strip_prefix("**")?.split_once(":**")?;
    if label.contains('*') {
        return None;
    }
    let value = after.strip_prefix(' ').unwrap_or(after).trim();
    Some((label.trim().to_owned(), value.to_owned()))
}

/// A slug proven to be kebab-case on the way in, so a newline or a
/// space cannot write a second heading or an id `remove_entry`
/// cannot match.
struct Slug(String);

impl Slug {
    fn new(raw: &str) -> Result<Self, String> {
        require_nonempty("slug", raw)?;
        if !is_slug(raw) {
            return Err(format!(
                "slug '{raw}' is not kebab-case; use lowercase letters, \
                 digits and dashes, the only shape docs/todo.md can be read \
                 back as"
            ));
        }
        Ok(Self(raw.to_owned()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// `<lead> <summary>` on one line of at most `width` chars. A wrapped
/// summary's second line would read as the body's first, so a long
/// one is refused, with its budget, rather than wrapped. Interior
/// whitespace collapses, so a newline cannot splice in an entry.
fn summary_line(
    lead: &str,
    summary: &str,
    width: usize,
) -> Result<String, String> {
    let summary = summary.split_whitespace().collect::<Vec<_>>().join(" ");
    let lead_width = lead.chars().count() + 1;
    let line = format!("{lead} {summary}");
    let len = line.chars().count();
    if len > width {
        return Err(format!(
            "summary is too long: {len} columns, limit {width}; it must fit \
             on one line, so keep it to {} characters after '{lead}' and \
             move the detail into the body",
            width.saturating_sub(lead_width)
        ));
    }
    Ok(line)
}

/// The lines of a new entry: heading, blank, summary, then an
/// optional body wrapped at column 0.
fn entry_lines(
    slug: &str,
    summary: &str,
    body: Option<&str>,
) -> Result<Vec<String>, String> {
    let summary = summary_line("**Summary:**", summary, MARKDOWN_WIDTH)?;
    let mut out = vec![format!("### {slug}"), String::new(), summary];
    if let Some(body) = body {
        let lines = wrap_markdown(body, "", "", MARKDOWN_WIDTH);
        // The body sits at column 0, where a heading would be read
        // back as a new entry or end the one above.
        if let Some(bad) = lines.iter().find(|l| reads_as_heading(l)) {
            return Err(format!(
                "body line {bad:?} would read as a heading in docs/todo.md; \
                 reword it so no line starts with `#` and a space"
            ));
        }
        if !lines.is_empty() {
            out.push(String::new());
            out.extend(lines);
        }
    }
    Ok(out)
}

/// The end (exclusive) of the entry whose heading is at `idx`: the
/// next column-0 heading of any level, or the line count.
fn entry_end<S: AsRef<str>>(lines: &[S], idx: usize) -> usize {
    (idx + 1..lines.len())
        .find(|&i| reads_as_heading(lines[i].as_ref()))
        .unwrap_or(lines.len())
}

/// Refuse a file whose shape the queue cannot edit safely: after the
/// first item, every column-0 heading must be an item. The file is an
/// introduction, then items only; anything else there (a footer, a
/// prose subsection) is named rather than guessed around.
fn check_shape(content: &str) -> Result<(), String> {
    // The template's earlier layout (`## Pending` / `## Done` sections
    // of bullets) parses as no items at all, so every edit would land
    // in the wrong place without a word. It can still arrive: a
    // derived project keeps its own `docs/todo.md` across syncs.
    let old_layout = content
        .lines()
        .any(|l| matches!(l.trim_end(), "## Pending" | "## Done"));
    if old_layout {
        return Err("docs/todo.md uses the old Pending/Done layout; convert \
                    each pending bullet to an `### <slug>` heading with a \
                    `**Summary:** <one line>` field and its text as the \
                    body, and delete the Done section"
            .into());
    }
    let items = parse(content);
    let Some(first) = items.first() else {
        return Ok(());
    };
    let item_lines: Vec<usize> = items.iter().map(|r| r.idx).collect();
    let mut fenced = false;
    for (i, line) in content.lines().enumerate().skip(first.idx + 1) {
        if is_fence(line) {
            fenced = !fenced;
            continue;
        }
        // `add` never writes a heading inside a code block; one there
        // (a hand-pasted example entry) would split the item.
        if fenced && reads_as_heading(line) {
            return Err(format!(
                "docs/todo.md line {}: {line:?} is a heading inside a code \
                 block; remove it or indent the block",
                i + 1
            ));
        }
        if reads_as_heading(line) && !item_lines.contains(&i) {
            return Err(format!(
                "docs/todo.md line {}: {line:?} is not an item, but items \
                 come after it; move it above the first item or remove it",
                i + 1
            ));
        }
    }
    Ok(())
}

/// `content` with `entry` after its last non-empty line, a blank line
/// between (none in an empty file), so a new item lands at the tail.
fn append_entry(content: &str, entry: Vec<String>) -> String {
    let mut lines = to_owned_lines(content);
    let at = (0..lines.len())
        .rev()
        .find(|&i| !lines[i].trim().is_empty())
        .map_or(0, |i| i + 1);
    let mut ins = if at == 0 {
        Vec::new()
    } else {
        vec![String::new()]
    };
    ins.extend(entry);
    lines.splice(at..at, ins);
    finish(lines, content)
}

/// `content` without the entry for `slug`: from its heading to the
/// next column-0 heading (the next item, by `check_shape`) or the
/// end. Trailing blank lines are trimmed, so the file still ends with
/// one newline.
fn remove_entry(content: &str, slug: &Slug) -> Result<String, String> {
    let entry = parse(content)
        .into_iter()
        .find(|r| r.id == slug.as_str())
        .ok_or_else(|| format!("no todo with slug '{}'", slug.as_str()))?;
    let mut lines = to_owned_lines(content);
    let end = entry_end(&lines, entry.idx);
    lines.splice(entry.idx..end, std::iter::empty());
    Ok(finish(lines, content))
}

/// `lines` rejoined in `content`'s line ending, trailing blank lines
/// dropped, ending with exactly one line ending.
fn finish(mut lines: Vec<String>, content: &str) -> String {
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    let mut out = rejoin_like(&lines, content);
    if !lines.is_empty() && !out.ends_with('\n') {
        out.push_str(if content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_repo;

    const SAMPLE: &str = "\
# TODO

Project work queue.

### alpha-task

**Summary:** do alpha

more about alpha

### beta-task

**Summary:** do beta
";

    fn slug(s: &str) -> Slug {
        Slug::new(s).unwrap()
    }

    #[test]
    fn parse_reads_slugs_summaries_and_skips_body_bold_text() {
        let got: Vec<(String, Option<String>)> = parse(SAMPLE)
            .into_iter()
            .map(|r| (r.id, r.summary))
            .collect();
        assert_eq!(
            got,
            [
                ("alpha-task".to_owned(), Some("do alpha".to_owned())),
                ("beta-task".to_owned(), Some("do beta".to_owned())),
            ]
        );
        // Only the field block counts: a later bold `Summary:` is prose.
        let two = parse("### a\n\n**Summary:** s\n\n**Summary:** prose\n");
        assert_eq!(two[0].summary.as_deref(), Some("s"));
        assert_eq!(parse_field("**Not a field**: x"), None);
        assert_eq!(parse_field("**Bold `x`.**"), None);
    }

    #[test]
    fn slugs_must_be_kebab_case() {
        for bad in
            ["", "Has-Caps", "two words", "a--b", "-a", "a\n### b", "a_b"]
        {
            assert!(Slug::new(bad).is_err(), "{bad:?}");
        }
        assert!(Slug::new("rt-2026-10-20-x").is_ok());
    }

    #[test]
    fn a_summary_must_fit_one_line() {
        let got = summary_line("**Summary:**", "a\n### ghost", 80).unwrap();
        assert_eq!(got, "**Summary:** a ### ghost");
        let e = summary_line("**Summary:**", &"x".repeat(80), 80).unwrap_err();
        assert!(e.contains("too long") && e.contains("67"), "{e}");
        assert!(summary_line("**Summary:**", &"é".repeat(20), 40).is_ok());
    }

    #[test]
    fn an_entry_has_its_heading_summary_and_wrapped_body() {
        let got =
            entry_lines("gamma", "do gamma", Some("a longer note")).unwrap();
        assert_eq!(
            got,
            [
                "### gamma",
                "",
                "**Summary:** do gamma",
                "",
                "a longer note"
            ]
        );
        let bare = entry_lines("gamma", "do gamma", None).unwrap();
        assert_eq!(bare.len(), 3);
        // A long body wraps at the width, at column 0, after a blank.
        let long = entry_lines("g", "s", Some(&"xx ".repeat(60))).unwrap();
        assert_eq!(long[3], "");
        assert!(long.len() > 5, "{long:?}");
        assert!(
            long[4..]
                .iter()
                .all(|l| l.len() <= MARKDOWN_WIDTH && l.starts_with('x')),
            "{long:?}"
        );
        let e = entry_lines("g", &"x".repeat(80), None).unwrap_err();
        assert!(e.contains("body"), "{e}");
    }

    #[test]
    fn a_hash_that_is_not_a_heading_is_allowed_in_a_body() {
        // `#7` wrapped to a line start and `#hashtag` are not
        // headings, so neither is refused.
        for body in ["tracked in #7 and more", "#hashtag here"] {
            assert!(entry_lines("g", "s", Some(body)).is_ok(), "{body:?}");
        }
        let wraps = format!("{} #7 follows", "word ".repeat(16));
        let lines = entry_lines("g", "s", Some(&wraps)).unwrap();
        assert!(lines.iter().any(|l| l.starts_with("#7")), "{lines:?}");
    }

    #[test]
    fn a_non_item_heading_after_the_first_item_is_refused() {
        // The file is an introduction, then items only: a footer or a
        // prose subsection among items is named, not guessed around.
        for doc in [
            format!("{SAMPLE}\n## Notes\n\nfooter prose\n"),
            "### a\n\n**Summary:** s\n\n### notes\n\nprose\n".to_owned(),
        ] {
            let e = check_shape(&doc).unwrap_err();
            assert!(e.contains("line") && e.contains("not an item"), "{e}");
        }
        // Headings before the first item are introduction.
        let intro = format!(
            "# TODO\n\n## How to use\n\nprose\n\n{}",
            SAMPLE.strip_prefix("# TODO\n\n").unwrap()
        );
        assert_eq!(check_shape(&intro), Ok(()));
        assert_eq!(check_shape("# TODO\n\n## Notes\n"), Ok(()));
    }

    #[test]
    fn a_heading_without_a_summary_is_prose_not_an_item() {
        let doc = "### notes\n\nprose\n\n### real-one\n\n**Summary:** s\n";
        let ids: Vec<String> = parse(doc).into_iter().map(|r| r.id).collect();
        assert_eq!(ids, ["real-one"]);
    }

    #[test]
    fn only_slug_headings_are_items() {
        let doc = "# TODO\n\n### Some Notes\n\nprose\n\n### real-one\n\n**Summary:** s\n";
        let ids: Vec<String> = parse(doc).into_iter().map(|r| r.id).collect();
        assert_eq!(ids, ["real-one"]);
    }

    #[test]
    fn the_old_pending_done_layout_is_refused_with_a_way_out() {
        for doc in ["# TODO\n\n## Pending\n\n- **a** -- do a\n", "## Done\n"] {
            let e = check_shape(doc).unwrap_err();
            assert!(e.contains("old") && e.contains("layout"), "{e}");
        }
    }

    #[test]
    fn a_bold_bullet_in_a_body_is_not_the_old_layout() {
        let doc = "### a\n\n**Summary:** s\n\n- **Note** -- x\n";
        assert_eq!(check_shape(doc), Ok(()));
    }

    #[test]
    fn a_heading_inside_a_code_block_after_the_first_item_is_refused() {
        let doc = "### a\n\n**Summary:** s\n\n```md\n### example\n\n**Summary:** x\n```\n";
        let e = check_shape(doc).unwrap_err();
        assert!(e.contains("code block"), "{e}");
    }

    #[test]
    fn edits_always_end_the_file_with_one_newline() {
        let only = "### a\n\n**Summary:** s\n";
        let emptied = remove_entry(only, &slug("a")).unwrap();
        let out = append_entry(&emptied, entry_lines("b", "t", None).unwrap());
        assert_eq!(out, "### b\n\n**Summary:** t\n");
        let no_nl =
            append_entry("# T\n\nintro", entry_lines("b", "t", None).unwrap());
        assert!(no_nl.ends_with("**Summary:** t\n"), "{no_nl:?}");
    }

    #[test]
    fn removing_the_last_item_ends_the_file_cleanly() {
        let last = remove_entry(SAMPLE, &slug("beta-task")).unwrap();
        assert!(last.ends_with("more about alpha\n"), "{last:?}");
    }

    #[test]
    fn the_first_item_in_an_empty_file_is_written_cleanly() {
        let out = append_entry("", entry_lines("x", "y", None).unwrap());
        assert_eq!(out, "### x\n\n**Summary:** y\n");
    }

    #[test]
    fn a_body_line_that_would_read_as_a_heading_is_refused() {
        // At column 0, `### ` would start a new entry when read back.
        // Code blocks included: the body is at column 0, and removal
        // ends an item at any heading-shaped line.
        for body in [
            "### not an entry",
            "intro\n\n## Section",
            "```sh\n# a comment\n```",
        ] {
            let e = entry_lines("g", "s", Some(body)).unwrap_err();
            assert!(e.contains("heading"), "{body:?}: {e}");
        }
    }

    #[test]
    fn append_lands_after_the_last_entry_and_remove_takes_one_whole() {
        let entry = entry_lines("gamma", "do gamma", None).unwrap();
        let out = append_entry(SAMPLE, entry);
        assert!(
            out.ends_with(
                "**Summary:** do beta\n\n### gamma\n\n**Summary:** do gamma\n"
            ),
            "{out}"
        );
        let gone = remove_entry(&out, &slug("alpha-task")).unwrap();
        assert!(
            !gone.contains("alpha") && gone.contains("### beta-task"),
            "{gone}"
        );
        assert!(
            gone.contains("Project work queue.\n\n### beta-task"),
            "{gone}"
        );
        let last = remove_entry(&out, &slug("gamma")).unwrap();
        assert!(last.ends_with("**Summary:** do beta\n"), "{last:?}");
        let e = remove_entry(SAMPLE, &slug("missing")).unwrap_err();
        assert!(e.contains("missing"), "{e}");
    }

    #[test]
    fn crlf_files_keep_their_endings() {
        let crlf = SAMPLE.replace('\n', "\r\n");
        let out = append_entry(&crlf, entry_lines("g", "s", None).unwrap());
        assert!(!out.replace("\r\n", "").contains('\n'), "{out:?}");
    }

    #[test]
    fn list_add_and_done_work_on_the_workspace_file() {
        let dir = test_repo::empty_dir();
        std::fs::create_dir(dir.0.join("docs")).unwrap();
        std::fs::write(dir.0.join("docs/todo.md"), SAMPLE).unwrap();
        let cfg = Config::for_project("p", dir.0.clone());
        add(&cfg, "gamma", "do gamma", Some("why")).unwrap();
        let e = add(&cfg, "gamma", "again", None).unwrap_err();
        assert!(e.contains("already exists"), "{e}");
        done(&cfg, "alpha-task").unwrap();
        let items = list(&cfg).unwrap();
        let slugs: Vec<&str> = items.iter().map(|i| i.slug.as_str()).collect();
        assert_eq!(slugs, ["beta-task", "gamma"]);
        assert_eq!(items[1].summary, "do gamma");
        assert!(done(&cfg, "alpha-task").is_err());
        let missing = Config::for_project("p", dir.0.join("nope"));
        assert!(list(&missing).unwrap_err().contains("read"));
    }
}
