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
//! The file holds live work only. Completing an item removes it
//! whole; what shipped is recorded by the commit and git history.

use std::fs;
use std::path::PathBuf;

use crate::Config;
use crate::markdown::{
    MARKDOWN_WIDTH, rejoin_like, require_nonempty, to_owned_lines,
    wrap_markdown,
};

/// A queued item, as `list` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The kebab-case slug its heading carries.
    pub slug: String,
    /// The `**Summary:**` line, empty when the entry has none.
    pub summary: String,
}

/// The queued items, in file order.
///
/// # Errors
///
/// When `docs/todo.md` cannot be read.
pub fn list(cfg: &Config) -> Result<Vec<Item>, String> {
    Ok(parse(&read(cfg)?)
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
/// or unwritable `docs/todo.md`.
pub fn add(
    cfg: &Config,
    slug: &str,
    summary: &str,
    body: Option<&str>,
) -> Result<(), String> {
    let slug = Slug::new("todo --slug", slug)?;
    require_nonempty("todo --summary", summary)?;
    // Rendered before the file is read, so a bad argument fails the
    // same way with or without a readable `docs/todo.md`.
    let entry = entry_lines(slug.as_str(), summary, body)?;
    let content = read(cfg)?;
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
/// or unwritable `docs/todo.md`.
pub fn done(cfg: &Config, slug: &str) -> Result<(), String> {
    let slug = Slug::new("todo done <slug>", slug)?;
    let content = read(cfg)?;
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

/// One parsed entry.
struct Record {
    /// The slug, from the heading up to a ` -- ` separator.
    id: String,
    /// 1-indexed line of the `### ` heading.
    line: usize,
    /// The `**Summary:**` field value, if the entry has one.
    summary: Option<String>,
}

/// The entries of a queue file. An entry opens with an `### ` heading
/// at column 0; its field block is the run of `**Label:** value`
/// lines directly under it (after an optional blank line), ending at
/// the first blank or non-field line.
fn parse(content: &str) -> Vec<Record> {
    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(rest) = lines[i].strip_prefix("### ") else {
            i += 1;
            continue;
        };
        let rest = rest.trim();
        let id = rest.split_once(" -- ").map_or(rest, |(id, _)| id.trim());
        let mut record = Record {
            id: id.to_owned(),
            line: i + 1,
            summary: None,
        };
        let mut j = i + 1;
        if j < lines.len() && lines[j].trim().is_empty() {
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
        out.push(record);
        i = j.max(i + 1);
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

/// A slug proven to be kebab-case, the one shape the file reads back:
/// a newline or a space in it would write a second heading or an id
/// `remove_entry` cannot match.
struct Slug(String);

impl Slug {
    /// `what` names the argument in the message: `add` takes a
    /// `--slug` flag, `done` a positional slug.
    fn new(what: &str, raw: &str) -> Result<Self, String> {
        require_nonempty(what, raw)?;
        let kebab = raw.split('-').all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
        if !kebab {
            return Err(format!(
                "{what} '{raw}' is not a slug; use lowercase letters, digits \
                 and dashes, the only shape docs/todo.md can be read back as"
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
             on one line, so keep it to {} characters after '{lead}'",
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
    let summary = summary_line("**Summary:**", summary, MARKDOWN_WIDTH)
        .map_err(|e| format!("{e}; move the detail into --body"))?;
    let mut out = vec![format!("### {slug}"), String::new(), summary];
    if let Some(body) = body {
        let lines = wrap_markdown(body, "", "", MARKDOWN_WIDTH);
        // The body sits at column 0, where `#` starts a heading: an
        // `### ` line would be read back as a new entry.
        if let Some(bad) = lines.iter().find(|l| l.starts_with('#')) {
            return Err(format!(
                "--body line {bad:?} would read as a heading in \
                 docs/todo.md; indent it or reword it"
            ));
        }
        if !lines.is_empty() {
            out.push(String::new());
            out.extend(lines);
        }
    }
    Ok(out)
}

/// `content` with `entry` after its last non-empty line, a blank
/// line between, so a new item lands at the tail.
fn append_entry(content: &str, entry: Vec<String>) -> String {
    let mut lines = to_owned_lines(content);
    let at = (0..lines.len())
        .rev()
        .find(|&i| !lines[i].trim().is_empty())
        .map_or(lines.len(), |i| i + 1);
    let mut ins = vec![String::new()];
    ins.extend(entry);
    lines.splice(at..at, ins);
    rejoin_like(&lines, content)
}

/// `content` without the entry for `slug`: from its heading to the
/// next entry's heading (or the end), so a multi-line entry goes
/// whole and the blank before the next heading stays as a separator.
fn remove_entry(content: &str, slug: &Slug) -> Result<String, String> {
    let records = parse(content);
    let idx = records
        .iter()
        .position(|r| r.id == slug.as_str())
        .ok_or_else(|| format!("no todo with slug '{}'", slug.as_str()))?;
    let mut lines = to_owned_lines(content);
    let start = records[idx].line - 1;
    let end = records
        .get(idx + 1)
        .map_or(lines.len(), |next| next.line - 1);
    lines.splice(start..end, std::iter::empty());
    Ok(rejoin_like(&lines, content))
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
        Slug::new("x", s).unwrap()
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
        let titled = parse(
            "### tf-1 -- a title\n\n**Summary:** s\n\n**Summary:** prose\n",
        );
        assert_eq!(titled[0].id, "tf-1");
        assert_eq!(titled[0].summary.as_deref(), Some("s"));
        assert_eq!(parse_field("**Not a field**: x"), None);
        assert_eq!(parse_field("**Bold `x`.**"), None);
    }

    #[test]
    fn slugs_must_be_kebab_case() {
        for bad in
            ["", "Has-Caps", "two words", "a--b", "-a", "a\n### b", "a_b"]
        {
            assert!(Slug::new("--slug", bad).is_err(), "{bad:?}");
        }
        assert!(Slug::new("--slug", "rt-2026-10-20-x").is_ok());
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
        let e = entry_lines("g", "s", Some(&"x".repeat(80).replace('x', "x ")))
            .map(|_| ());
        assert!(e.is_ok());
        let e = entry_lines("g", &"x".repeat(80), None).unwrap_err();
        assert!(e.contains("--body"), "{e}");
    }

    #[test]
    fn a_body_line_that_would_read_as_a_heading_is_refused() {
        // At column 0, `### ` would start a new entry when read back.
        for body in ["### not an entry", "intro\n\n## Section"] {
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
        assert!(last.ends_with("**Summary:** do beta\n\n"), "{last:?}");
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
