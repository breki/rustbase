//! Checks the claims canon prose makes about this repository.
//!
//! Canon is the reviewed, shared guidance -- `CLAUDE.md` and the
//! `.claude/` commands and agents -- plus, for the dangling-ID
//! check alone, the reference docs under `docs/`.
//!
//! Everything here reads files and compares strings; it decides
//! nothing about phrasing. `.claude/skills/` is unchecked, and a
//! green run says nothing about it.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use crate::Config;
use crate::text::is_iso_date;

/// One thing canon claims that the tree does not support.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    /// Short tag naming which check fired.
    kind: &'static str,
    /// Repo-relative file the claim is in.
    file: String,
    /// 1-indexed line the claim is on.
    line: usize,
    /// What is wrong, in one clause.
    message: String,
}

/// A path canon names that is correct to be absent, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exempt {
    /// Covers exactly this path, or, ending in `/`, everything in
    /// that folder; `crates/a` does not cover `crates/a-web/x.rs`.
    pub path: String,
    /// Required: an unexplained entry is indistinguishable from a
    /// defect somebody silenced, so a blank one fails the gate.
    pub reason: String,
}

impl Exempt {
    /// An exemption for `path`, because of `reason`.
    pub fn new(path: &str, reason: &str) -> Self {
        Self {
            path: path.to_owned(),
            reason: reason.to_owned(),
        }
    }
}

/// What differs between projects in the canon check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    /// Paths canon names that are correct to be absent. None by
    /// default; a project lists its own.
    pub absent_on_purpose: Vec<Exempt>,
    /// Directory prefixes that make a backticked string a claim
    /// about a file in the repository rather than prose.
    pub repo_prefixes: Vec<String>,
    /// Files and subtrees (a trailing `/`) under `docs/` whose
    /// backlog-ID citations are not checked: records that cite IDs
    /// as data, not pointers meant to resolve. The defaults are the
    /// reviewer logs, the template-feedback file and the working
    /// issue docs every rustbase-derived project keeps.
    pub citation_exempt: Vec<String>,
    /// Maximum prose width. Defaults to 80, the width `rustfmt.toml`
    /// sets; it is not read from that file.
    pub max_cols: usize,
}

impl Rules {
    /// Refuse exemptions that would quietly weaken the check. The
    /// checked folders are the `repo_prefixes` for an absent path and
    /// `docs/` for a citation entry. An entry is refused when it is
    /// blank, names a checked folder itself (exempting all of it), or
    /// lies outside every checked folder (matching nothing).
    ///
    /// It does not refuse an empty `repo_prefixes` or a large
    /// `max_cols`; either one switches a check off. Nor can it tell
    /// that an entry inside a folder names a file canon never cites.
    ///
    /// # Errors
    ///
    /// The first entry that is blank, has no reason, does not name
    /// something inside one of the checked folders, or a repo prefix
    /// that does not end in `/`.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(p) = self.repo_prefixes.iter().find(|p| !p.ends_with('/')) {
            return Err(format!(
                "canon repo prefix {p:?} must end in `/`, so it names a folder"
            ));
        }
        let inside = |path: &str, folder: &str| {
            path.starts_with(folder) && path.len() > folder.len()
        };
        for e in &self.absent_on_purpose {
            if e.path.trim().is_empty() {
                return Err(format!(
                    "canon exemption with reason {:?} names no path",
                    e.reason
                ));
            }
            if e.reason.trim().is_empty() {
                return Err(format!(
                    "canon exemption `{}` gives no reason; say why it is \
                     absent",
                    e.path
                ));
            }
            if !self.repo_prefixes.iter().any(|p| inside(&e.path, p)) {
                return Err(format!(
                    "canon exemption `{}` must name a path inside one of \
                     {:?}: the folder itself would exempt all of it, and \
                     anything outside would match nothing",
                    e.path, self.repo_prefixes
                ));
            }
        }
        if let Some(c) =
            self.citation_exempt.iter().find(|c| !inside(c, "docs/"))
        {
            return Err(format!(
                "canon citation exemption {c:?} must name a file or folder \
                 inside docs/: docs/ itself would exempt every doc, and \
                 anything outside would match nothing"
            ));
        }
        Ok(())
    }
}

impl Default for Rules {
    fn default() -> Self {
        let owned =
            |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect();
        Self {
            absent_on_purpose: Vec::new(),
            repo_prefixes: owned(&[
                "crates/", "xtask/", "docs/", "scripts/", ".claude/",
            ]),
            citation_exempt: owned(&[
                "docs/developer/redteam-log.md",
                "docs/developer/artisan-log.md",
                "docs/developer/fresh-reader-log.md",
                "docs/developer/template-feedback.md",
                "docs/issues/",
            ]),
            max_cols: 80,
        }
    }
}

/// How many checks run, for the summary line only. Nothing
/// compares it against `collect`, so bump it by hand when a
/// check is added: a stale value misreports the count and
/// breaks nothing else.
const CHECK_COUNT: usize = 5;

/// Text between the first `open` and the next `close` after it,
/// with the byte index the match started at.
fn between<'a>(
    hay: &'a str,
    open: &str,
    close: &str,
) -> Option<(usize, &'a str)> {
    let start = hay.find(open)?;
    let rest = &hay[start + open.len()..];
    let end = rest.find(close)?;
    Some((start, &rest[..end]))
}

/// A paragraph, one list item or one heading, with its lines
/// joined.
///
/// Every markdown file here wraps at 80 columns, so a bold
/// lead-in or a cross-reference is regularly split across two
/// lines and a line-by-line search never sees it. `text` holds
/// the lines trimmed and joined with a single space, and
/// `first_line` with `more` remembers which source line each
/// piece came from, so a finding still names a line the
/// author can open.
struct Block {
    /// The trimmed lines, joined with one space.
    text: String,
    /// The 1-indexed source line the block opens with. It is
    /// a field rather than the first `more` entry so that
    /// `line_at` always has an answer to give.
    first_line: usize,
    /// One entry per line after the first: the byte offset
    /// where that line begins in `text`, and its 1-indexed
    /// source line.
    more: Vec<(usize, usize)>,
}

impl Block {
    /// A block holding one line.
    fn new(first_line: usize, text: &str) -> Self {
        Self {
            text: text.to_string(),
            first_line,
            more: Vec::new(),
        }
    }

    /// Append a wrapped continuation line.
    fn push(&mut self, source_line: usize, text: &str) {
        self.text.push(' ');
        self.more.push((self.text.len(), source_line));
        self.text.push_str(text);
    }

    /// Where each joined line begins in `text`.
    fn line_offsets(&self) -> impl Iterator<Item = usize> + '_ {
        std::iter::once(0).chain(self.more.iter().map(|(at, _)| *at))
    }

    /// The 1-indexed source line that byte `offset` fell in.
    fn line_at(&self, offset: usize) -> usize {
        self.more
            .iter()
            .rev()
            .find(|(at, _)| *at <= offset)
            .map_or(self.first_line, |(_, line)| *line)
    }
}

/// The text after a `- `, `* `, `+ ` or `12. ` list marker, or
/// the whole line when it carries none.
fn strip_list_marker(line: &str) -> &str {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return rest.trim_start();
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && line[digits..].starts_with(". ") {
        return line[digits + 2..].trim_start();
    }
    line
}

/// Split the file into blocks of consecutive non-blank lines.
///
/// A blank line, a heading and a new list item each end the
/// block before them. Ending at a blank line is what stops a
/// `**` left open in one paragraph from closing in the next
/// and inventing a target that spans both.
///
/// A heading is a block of its own: it ends the block before
/// it and is closed straight away, so its text never joins
/// the paragraph beneath it. Without that, a `**` opened in a
/// heading pairs with one in the prose below and the checker
/// reports a pointer nobody wrote.
fn blocks(content: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut cur: Option<Block> = None;
    for (i, raw) in content.lines().enumerate() {
        let line = raw.trim();
        let heading = line.starts_with('#');
        let breaks = line.is_empty()
            || heading
            || strip_list_marker(line).len() != line.len();
        if breaks && let Some(block) = cur.take() {
            out.push(block);
        }
        if line.is_empty() {
            continue;
        }
        if heading {
            out.push(Block::new(i + 1, line));
            continue;
        }
        match &mut cur {
            Some(block) => block.push(i + 1, line),
            None => cur = Some(Block::new(i + 1, line)),
        }
    }
    out.extend(cur);
    out
}

/// Every `**Bold**` run that opens a line, after any list
/// marker, plus every heading. These are what a
/// cross-reference may name.
///
/// A line, not a paragraph: a rule as often opens the third
/// line of a paragraph as the first, and a bold run that
/// merely lands at a line start by an accident of wrapping
/// becomes citable too. Accepting those costs nothing -- a
/// target nobody names is never looked up -- while missing
/// one fails a correct pointer.
fn reference_targets(blocks: &[Block]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for block in blocks {
        // `blocks` closes a heading as a block of its own, so
        // its text is the whole heading line.
        if let Some(rest) = block.text.strip_prefix('#') {
            let title = rest.trim_start_matches('#').trim();
            // `### 2. Snapshot` is referred to as **Snapshot**.
            let title = title.split_once(". ").map_or(title, |(head, tail)| {
                if head.chars().all(|c| c.is_ascii_digit()) {
                    tail
                } else {
                    title
                }
            });
            out.insert(title.to_string());
            continue;
        }
        // Scanning from a line start rather than from the
        // whole block is what lets a `**` run that wraps
        // still close.
        for offset in block.line_offsets() {
            let lead = strip_list_marker(&block.text[offset..]);
            if lead.starts_with("**")
                && let Some((_, inner)) = between(lead, "**", "**")
            {
                out.insert(inner.trim_end_matches('.').to_string());
            }
        }
    }
    out
}

/// Cross-references of the form `under **X**` whose `X` names
/// no heading and no bold-led paragraph anywhere in canon.
///
/// Only `under` is scanned. That is the form these files
/// actually use for a pointer, and widening it to `in` or `at`
/// matches ordinary prose such as "the only one asked what
/// **worked**".
fn unresolved_xrefs(
    file: &str,
    blocks: &[Block],
    targets: &BTreeSet<String>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for block in blocks {
        let mut rest = block.text.as_str();
        let mut base = 0;
        while let Some((at, inner)) = between(rest, "under **", "**") {
            if !targets.contains(inner) {
                out.push(Finding {
                    kind: "xref",
                    file: file.to_string(),
                    line: block.line_at(base + at),
                    message: format!("**{inner}** names no heading"),
                });
            }
            // Resume just past `under **` rather than past
            // the closing `**`. An unclosed run pairs with
            // the *opening* `**` of the next pointer, and
            // skipping the pair would swallow that pointer's
            // `under **` and stop checking it.
            let step = at + "under **".len();
            base += step;
            rest = &rest[step..];
        }
    }
    out
}

/// Backticked repository paths that do not exist.
///
/// `exists` is injected so the tests need no files on disk.
fn missing_paths(
    file: &str,
    content: &str,
    exists: &dyn Fn(&str) -> bool,
    rules: &Rules,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        for span in line.split('`').skip(1).step_by(2) {
            let p = span.trim();
            if !rules
                .repo_prefixes
                .iter()
                .any(|d| p.starts_with(d.as_str()))
            {
                continue;
            }
            // A glob, a placeholder such as `<slug>` and a
            // bare directory are all claims about a shape
            // rather than about one file.
            if p.contains('*') || p.contains('<') || p.ends_with('/') {
                continue;
            }
            if rules.absent_on_purpose.iter().any(|a| covers(&a.path, p)) {
                continue;
            }
            if !exists(p) {
                out.push(Finding {
                    kind: "path",
                    file: file.to_string(),
                    line: i + 1,
                    message: format!("`{p}` does not exist"),
                });
            }
        }
    }
    out
}

/// `git` subcommands a command file tells the agent to run that
/// its own `allowed-tools` does not grant.
///
/// Every backticked `git` command counts, including one inside a
/// prose quotation: a file that only prints one for the operator
/// says so with ``no `git <sub>` grant``, which exempts that
/// subcommand file-wide. A chained span (`&&`, `||`, `;`, `|`)
/// is checked per command. A best-effort line scan: fenced
/// blocks, spans wrapping across lines and `git` after `$(` or a
/// `VAR=` prefix are not seen, and a wildcard-less grant is read
/// as a prefix, so it covers more than it names.
fn ungranted_git(file: &str, content: &str) -> Vec<Finding> {
    let Some(grants) =
        content.lines().find(|l| l.starts_with("allowed-tools:"))
    else {
        return Vec::new();
    };
    // A `git` mention in the prose may wrap across lines, so
    // match it against the text with every whitespace run
    // collapsed. (The `allowed-tools:` grant read above is one
    // line.)
    let flat = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let granted = GitGrants::parse(grants);
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if line.starts_with("allowed-tools:") {
            continue;
        }
        let commands = line
            .split('`')
            .skip(1)
            .step_by(2)
            .flat_map(|span| span.split([';', '|', '&']))
            .map(str::trim);
        for cmd in commands {
            let Some(rest) = cmd.strip_prefix("git ") else {
                continue;
            };
            let mut words = rest.split_whitespace();
            let sub = words.next().unwrap_or_default();
            if sub.is_empty() || sub.starts_with('<') {
                continue;
            }
            if granted.covers(cmd) {
                continue;
            }
            let shown = match words.next() {
                Some(w) if !w.starts_with(['-', '<', '"']) => {
                    format!("git {sub} {w}")
                }
                _ => format!("git {sub}"),
            };
            // A file may state that it deliberately lacks a
            // grant, when the command is one it tells the
            // operator to run rather than running it. The
            // declaration is in the prose, so it greps.
            if flat.contains(&format!("no `git {sub}` grant")) {
                continue;
            }
            out.push(Finding {
                kind: "grant",
                file: file.to_string(),
                line: i + 1,
                message: format!("runs `{shown}`, not in allowed-tools"),
            });
        }
    }
    out
}

/// What an `allowed-tools:` line grants of `git`.
struct GitGrants {
    /// A whole-git grant: bare `Bash`, `Bash(*)`, `Bash(git:*)`
    /// or `Bash(git *)`.
    all: bool,
    /// Each narrower grant's command text without its `:*` / `*`
    /// wildcard: `Bash(git stash list:*)` gives `git stash list`.
    prefixes: Vec<String>,
}

impl GitGrants {
    fn parse(line: &str) -> Self {
        let mut grants = Self {
            all: false,
            prefixes: Vec::new(),
        };
        let list = line.strip_prefix("allowed-tools:").unwrap_or(line);
        for tool in list.split(',').map(str::trim) {
            if tool == "Bash" {
                grants.all = true;
                continue;
            }
            let Some(inner) =
                tool.strip_prefix("Bash(").and_then(|t| t.strip_suffix(')'))
            else {
                continue;
            };
            let cmd = inner
                .strip_suffix(":*")
                .or_else(|| inner.strip_suffix('*'))
                .unwrap_or(inner)
                .trim();
            if cmd.is_empty() || cmd == "git" {
                grants.all = true;
            } else if cmd.starts_with("git ") {
                grants.prefixes.push(cmd.to_string());
            }
        }
        grants
    }

    /// True when `cmd` is a granted prefix, whole or followed by
    /// a word boundary.
    fn covers(&self, cmd: &str) -> bool {
        self.all
            || self.prefixes.iter().any(|g| {
                cmd == g.as_str()
                    || cmd
                        .strip_prefix(g.as_str())
                        .is_some_and(|tail| tail.starts_with(' '))
            })
    }
}

/// Prose lines wider than `max_cols` columns.
///
/// Frontmatter values, table rows and fenced or indented code
/// are exempt: none of them can be wrapped without changing
/// what they mean.
fn over_wide(file: &str, content: &str, max_cols: usize) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut in_front = false;
    let mut in_fence = false;
    for (i, line) in content.lines().enumerate() {
        if i == 0 && line.trim() == "---" {
            in_front = true;
            continue;
        }
        if in_front {
            if line.trim() == "---" {
                in_front = false;
            }
            continue;
        }
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence
            || line.starts_with('|')
            || line.starts_with("    ")
            || line.starts_with('\t')
        {
            continue;
        }
        if line.chars().count() > max_cols {
            out.push(Finding {
                kind: "cols",
                file: file.to_string(),
                line: i + 1,
                message: format!("{} columns", line.chars().count()),
            });
        }
    }
    out
}

/// Backlog IDs cited in prose that appear in no backlog.
///
/// A backlog ID (`rt-`/`aq-`/`fr-` plus a date and slug, minted by
/// a reviewer log under `docs/developer/`) is meant to grep to its
/// entry, so a reader can follow a citation to the finding it
/// names; a citation that greps to nothing defeats that.
fn unknown_ids(
    file: &str,
    content: &str,
    known: &BTreeSet<String>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        for word in line.split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        {
            if !is_backlog_id(word) || known.contains(word) {
                continue;
            }
            out.push(Finding {
                kind: "id",
                file: file.to_string(),
                line: i + 1,
                message: format!("{word} is in no backlog"),
            });
        }
    }
    out
}

/// True for a backlog ID: `rt-` (red-team), `aq-` (artisan) or
/// `fr-` (fresh-reader), then an ISO date and a non-empty slug.
fn is_backlog_id(word: &str) -> bool {
    let Some(rest) = ["rt-", "aq-", "fr-"]
        .iter()
        .find_map(|p| word.strip_prefix(*p))
    else {
        return false;
    };
    let (Some(date), Some(slug)) = (rest.get(..10), rest.get(10..)) else {
        return false;
    };
    is_iso_date(date) && slug.len() > 1 && slug.starts_with('-')
}

/// The backlog ids of a backlog file's entries, read by the records
/// parser so this check and `records-check` agree on what an entry
/// is.
fn ids_in_backlog(content: &str) -> BTreeSet<String> {
    crate::records::parse(content)
        .into_iter()
        .map(|e| e.id)
        .filter(|id| is_backlog_id(id))
        .collect()
}

/// True when exemption `entry` covers `path`: the same path, or,
/// for an entry ending in `/`, anything in that folder. Whole names
/// only, so `crates/a` does not cover `crates/a-web/x.rs`.
fn covers(entry: &str, path: &str) -> bool {
    path == entry || (entry.ends_with('/') && path.starts_with(entry))
}

/// True for a repo-relative path the citation scan skips. The
/// skipped files cite IDs as data and provenance (a finding about
/// an ID that does not grep, a plan discussing one), so a citation
/// there is not a live pointer.
fn is_citation_exempt(rel: &str, rules: &Rules) -> bool {
    rules.citation_exempt.iter().any(|e| covers(e, rel))
}

/// Markdown files under `docs/`, repo-relative and sorted, that the
/// dangling-ID citation scan reads: every `.md` under `docs/` not
/// exempted by `rules`, so the reference docs but not the record
/// or working files.
fn doc_citation_files(root: &Path, rules: &Rules) -> Vec<String> {
    let mut out = Vec::new();
    collect_docs_md(&root.join("docs"), root, &mut out);
    out.retain(|f| !is_citation_exempt(f, rules));
    out.sort();
    out
}

/// Append every `.md` file under `dir`, repo-relative, recursively.
fn collect_docs_md(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_docs_md(&path, root, out);
        } else if path.extension().is_some_and(|x| x == "md")
            && let Ok(rel) = path.strip_prefix(root)
        {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Canon files, repo-relative, in a stable order.
fn canon_files(root: &Path) -> Vec<String> {
    let mut out = vec!["CLAUDE.md".to_string()];
    for dir in [".claude/commands", ".claude/agents"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let path = e.path();
                if path.extension()? != "md" {
                    return None;
                }
                let n = e.file_name().to_string_lossy().to_string();
                Some(format!("{dir}/{n}"))
            })
            .collect();
        names.sort();
        out.extend(names);
    }
    out.retain(|f| root.join(f).is_file());
    out
}

/// Run every canon check over the tree.
///
/// The findings come out grouped by check, then by file and
/// line, so two runs over an unchanged tree print the same
/// text.
///
/// Returns the findings and the number of files read.
fn collect(
    root: &Path,
    rules: &Rules,
) -> Result<(Vec<Finding>, usize), String> {
    rules.validate()?;
    let files = canon_files(root);

    // `reference_targets` and `unresolved_xrefs` both walk the
    // paragraph blocks, so each file is split into them once, as
    // it is read, and kept beside its path and text.
    let mut sources: Vec<(String, String, Vec<Block>)> = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(root.join(f))
            .map_err(|e| format!("cannot read {f}: {e}"))?;
        let file_blocks = blocks(&text);
        sources.push((f.clone(), text, file_blocks));
    }

    let mut targets = BTreeSet::new();
    for (_, _, file_blocks) in &sources {
        targets.extend(reference_targets(file_blocks));
    }

    // The reviewer logs every rustbase-derived project keeps; the
    // location is fixed, not part of `Rules`. Found by their
    // `-log.md` name, unlike `records-check`, which reads
    // `Config::record_files`: a new log goes in that list too.
    let mut known_ids = BTreeSet::new();
    if let Ok(entries) = std::fs::read_dir(root.join("docs/developer")) {
        for e in entries.filter_map(Result::ok) {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with("-log.md") {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(e.path()) {
                known_ids.extend(ids_in_backlog(&text));
            }
        }
    }

    let exists = |p: &str| root.join(p).exists();
    let mut findings = Vec::new();
    for (f, text, file_blocks) in &sources {
        findings.extend(unresolved_xrefs(f, file_blocks, &targets));
        findings.extend(missing_paths(f, text, &exists, rules));
        findings.extend(ungranted_git(f, text));
        findings.extend(over_wide(f, text, rules.max_cols));
        findings.extend(unknown_ids(f, text, &known_ids));
    }

    // The dangling-ID citation check alone widens to docs/, so a
    // citation left dangling by a backlog sweep is caught wherever
    // it lives, not only in canon. The other four checks stay
    // canon-only.
    let mut files_read = sources.len();
    for f in doc_citation_files(root, rules) {
        let Ok(text) = std::fs::read_to_string(root.join(&f)) else {
            continue;
        };
        findings.extend(unknown_ids(&f, &text, &known_ids));
        files_read += 1;
    }

    findings.sort();
    Ok((findings, files_read))
}

/// The findings rendered one per line, for an error message.
fn render(findings: &[Finding]) -> String {
    let mut msg = String::new();
    for f in findings {
        let _ =
            writeln!(msg, "  [{}] {}:{} {}", f.kind, f.file, f.line, f.message);
    }
    let _ = write!(msg, "{} canon finding(s)", findings.len());
    msg
}

/// Runs the checks over the workspace with the project's
/// [`Rules`]; on success, a one-line summary (`29 files, 5
/// checks`).
///
/// # Errors
///
/// Every finding, one per line; a canon file that cannot be read;
/// or rules that [`Rules::validate`] refuses.
pub fn check(cfg: &Config) -> Result<String, String> {
    let (findings, files) = collect(&cfg.root, &cfg.canon)?;
    if findings.is_empty() {
        Ok(format!("{files} files, {CHECK_COUNT} checks"))
    } else {
        Err(render(&findings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_repo;

    fn targets(from: &str) -> BTreeSet<String> {
        reference_targets(&blocks(from))
    }

    fn xrefs(
        file: &str,
        content: &str,
        targets: &BTreeSet<String>,
    ) -> Vec<Finding> {
        unresolved_xrefs(file, &blocks(content), targets)
    }

    #[test]
    fn heading_and_bold_lead_are_both_reference_targets() {
        let t = targets("## When to run\n\n**Diff handoff.** x\n");
        assert!(t.contains("When to run"));
        assert!(t.contains("Diff handoff"));
    }

    #[test]
    fn numbered_heading_is_named_without_its_number() {
        let t = targets("### 2. Run it before anyone reads it\n");
        assert!(t.contains("Run it before anyone reads it"));
    }

    #[test]
    fn unresolved_xref_is_reported_and_resolved_one_is_not() {
        let t = targets("## Snapshot\n");
        let got = xrefs("a.md", "see under **Snapshot** ok", &t);
        assert!(got.is_empty(), "{got:?}");
        let bad = xrefs("a.md", "under **Nowhere** x", &t);
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].kind, "xref");
    }

    /// Ordinary prose puts bold after `in` and `at` constantly,
    /// so only the pointer form counts.
    #[test]
    fn bold_after_other_prepositions_is_not_a_reference() {
        let t = BTreeSet::new();
        let got = xrefs("a.md", "the only one asked what **worked**", &t);
        assert!(got.is_empty(), "{got:?}");
    }

    /// Every markdown file here wraps at 80 columns, so a
    /// bold lead-in longer than about 70 characters spans two
    /// lines. Reading it line by line makes it invisible.
    #[test]
    fn a_bold_lead_in_that_wraps_is_still_a_reference_target() {
        let t = targets(
            "**Ask before testing something that is\nnot the program.** Then x\n",
        );
        assert!(
            t.contains("Ask before testing something that is not the program")
        );
    }

    /// Most rules in `CLAUDE.md` are list items, so the bold
    /// lead sits behind a `- ` marker.
    #[test]
    fn a_list_item_bold_lead_is_a_reference_target() {
        let t = targets(
            "- **Do not grep canon prose for a\n  phrase.** Every file wraps.\n",
        );
        assert!(t.contains("Do not grep canon prose for a phrase"));
    }

    /// Joining must stop at a blank line, or a bold run left
    /// open in one paragraph closes in the next and invents a
    /// target spanning both.
    #[test]
    fn a_bold_run_does_not_close_across_a_paragraph_break() {
        let t = targets("**Unclosed lead\n\nand a later** bold\n");
        assert!(!t.contains("Unclosed lead and a later"));
    }

    /// A rule often opens a line partway through a paragraph,
    /// and is a target there too, not only on the first line.
    #[test]
    fn a_bold_lead_on_a_continuation_line_is_a_target() {
        let t = targets(
            "Prefer strong types. Avoid primitive obsession.\n**The representation has to be argued for.**\n`ScriptPath` is a checked `String`.\n",
        );
        assert!(t.contains("The representation has to be argued for"));
    }

    /// An unclosed `under **` pairs with the *opening* `**`
    /// of the next pointer. Resuming after that pair would
    /// eat the second pointer's `under **` and stop checking
    /// every pointer after it in the paragraph.
    #[test]
    fn an_unclosed_bold_does_not_hide_the_next_pointer() {
        let bad = xrefs(
            "a.md",
            "see under **Unclosed and\nalso under **Nope** here\n",
            &BTreeSet::new(),
        );
        assert_eq!(bad.len(), 2, "both pointers are checked");
        assert!(bad[1].message.contains("Nope"));
        assert_eq!(bad[1].line, 2);
    }

    /// A heading is not always followed by a blank line, and
    /// the bold rule under one is still a target.
    #[test]
    fn a_bold_lead_directly_under_a_heading_is_a_target() {
        let t = targets("## Rules\n**Say it once.** No preamble.\n");
        assert!(t.contains("Say it once"));
    }

    /// What the heading break is for: a heading's text must
    /// not join the paragraph beneath it, or a `**` opened in
    /// the heading pairs with one below and the checker
    /// reports a pointer nobody wrote.
    #[test]
    fn a_heading_does_not_join_the_paragraph_below_it() {
        let bad = xrefs(
            "a.md",
            "## Rules under **No\nsuch rule** x\n",
            &BTreeSet::new(),
        );
        assert!(bad.is_empty(), "got {bad:?}");
    }

    /// Two bullets with no blank line between them are one
    /// paragraph to a naive join, which would lose the second
    /// lead and merge the first into it.
    #[test]
    fn adjacent_list_items_stay_separate_targets() {
        let t = targets("- **First rule.** x\n- **Second rule.** y\n");
        assert!(t.contains("First rule"));
        assert!(t.contains("Second rule"));
    }

    #[test]
    fn list_markers_are_stripped_and_ordinary_text_is_not() {
        for (line, want) in [
            ("- item", "item"),
            ("* item", "item"),
            ("+ item", "item"),
            ("12. item", "item"),
            ("12 item", "12 item"),
            ("12.item", "12.item"),
            ("item", "item"),
        ] {
            assert_eq!(strip_list_marker(line), want, "on {line}");
        }
    }

    /// The second pointer in a joined paragraph has to be
    /// attributed to its own source line, not the first's.
    #[test]
    fn each_pointer_in_a_joined_paragraph_names_its_own_line() {
        let bad = xrefs(
            "a.md",
            "see under **Nope one** and\nalso under **Nope two** here\n",
            &BTreeSet::new(),
        );
        assert_eq!(bad.len(), 2);
        assert_eq!((bad[0].line, bad[1].line), (1, 2));
    }

    /// The citing side wraps too: a pointer split over two lines
    /// is still checked, and reported on the line it starts.
    #[test]
    fn a_wrapped_xref_is_checked_and_reported_on_its_own_line() {
        let t = targets("## Snapshot\n");
        let got = xrefs("a.md", "as it says\nunder **Snapshot**, do it\n", &t);
        assert!(got.is_empty(), "{got:?}");
        let bad = xrefs("a.md", "one\ntwo under **No\nsuch rule** x\n", &t);
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].message, "**No such rule** names no heading");
        assert_eq!(bad[0].line, 2);
    }

    #[test]
    fn only_the_quoted_crate_manifest_is_exempt() {
        let none = |_: &str| false;
        let f = missing_paths(
            "a.md",
            "see `crates/rustbase/src/gone.rs`",
            &none,
            &rules(),
        );
        assert_eq!(f.len(), 1, "{f:?}");
    }

    #[test]
    fn missing_repo_path_is_reported() {
        let none = |_: &str| false;
        let f =
            missing_paths("a.md", "see `xtask/src/gone.rs`", &none, &rules());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, "path");
    }

    /// The defaults plus two absent paths, the shape a project
    /// configures.
    fn rules() -> Rules {
        Rules {
            absent_on_purpose: vec![
                Exempt::new("scripts/e2e.sh", "pruned in CLI-only projects"),
                Exempt::new("crates/rustbase/Cargo.toml", "renamed downstream"),
            ],
            ..Rules::default()
        }
    }

    #[test]
    fn an_exemption_without_a_reason_fails_the_gate() {
        let dir = test_repo::empty_dir();
        let blank = Rules {
            absent_on_purpose: vec![Exempt::new("scripts/x.sh", "  ")],
            ..Rules::default()
        };
        let e = collect(&dir.0, &blank).unwrap_err();
        assert!(e.contains("scripts/x.sh") && e.contains("no reason"), "{e}");
        assert!(collect(&dir.0, &rules()).is_ok());
    }

    #[test]
    fn a_blank_entry_is_refused() {
        // A blank entry is a configuration mistake, whatever it would
        // match; it is refused rather than silently ignored.
        let blank_path = Rules {
            absent_on_purpose: vec![Exempt::new(" ", "a reason")],
            ..Rules::default()
        };
        let e = blank_path.validate().unwrap_err();
        assert!(e.contains("names no path"), "{e}");
        let mut blank_citation = Rules::default();
        blank_citation.citation_exempt.push(String::new());
        assert!(blank_citation.validate().is_err());
        let mut blank_prefix = Rules::default();
        blank_prefix.repo_prefixes.push("  ".into());
        assert!(blank_prefix.validate().is_err());
        assert!(rules().validate().is_ok());
    }

    #[test]
    fn an_entry_covers_its_exact_path_or_a_folder_ending_in_a_slash() {
        let none = |_: &str| false;
        let r = Rules {
            absent_on_purpose: vec![
                Exempt::new("crates/rustbase", "renamed downstream"),
                Exempt::new("scripts/old/", "removed folder"),
            ],
            ..Rules::default()
        };
        let skipped =
            |text: &str| missing_paths("a.md", text, &none, &r).is_empty();
        assert!(skipped("`crates/rustbase`"));
        assert!(skipped("`scripts/old/run.sh`"));
        // A name that merely starts with the same letters is not
        // covered.
        assert!(!skipped("`crates/rustbase-web/src/lib.rs`"));
        assert!(!skipped("`crates/rustbase/Cargo.toml`"));
        assert!(!skipped("`scripts/older.sh`"));

        let docs = Rules::default();
        assert!(is_citation_exempt("docs/issues/plan.md", &docs));
        assert!(!is_citation_exempt("docs/issues-old.md", &docs));
        let short = Rules {
            citation_exempt: vec!["docs/d".into()],
            ..Rules::default()
        };
        assert!(!is_citation_exempt("docs/developer/a.md", &short));
    }

    #[test]
    fn a_repo_prefix_must_end_in_a_slash() {
        let mut r = Rules::default();
        r.repo_prefixes.push("tools".into());
        let e = r.validate().unwrap_err();
        assert!(e.contains("tools") && e.contains("end in `/`"), "{e}");
    }

    #[test]
    fn an_entry_covering_a_whole_class_of_paths_is_refused() {
        // The folder itself (`crates/`, `docs/`) would exempt all of
        // it; anything else not strictly inside one matches nothing.
        for path in ["crates", "crates/", "c", ".claude"] {
            let r = Rules {
                absent_on_purpose: vec![Exempt::new(path, "a reason")],
                ..Rules::default()
            };
            let e = r.validate().unwrap_err();
            assert!(e.contains("inside one of"), "{path}: {e}");
        }
        let outside = Rules {
            absent_on_purpose: vec![Exempt::new("README.md", "never matches")],
            ..Rules::default()
        };
        assert!(outside.validate().is_err(), "an entry that cannot match");
        for entry in ["docs/", "docs", "d"] {
            let mut r = Rules::default();
            r.citation_exempt.push(entry.into());
            assert!(r.validate().is_err(), "{entry} exempts every doc");
        }
    }

    #[test]
    fn check_reads_canon_and_docs_from_the_workspace_root() {
        let dir = test_repo::empty_dir();
        std::fs::create_dir_all(dir.0.join(".claude/commands")).unwrap();
        std::fs::create_dir_all(dir.0.join("docs/issues")).unwrap();
        std::fs::write(dir.0.join("CLAUDE.md"), "# Title\n").unwrap();
        std::fs::write(dir.0.join(".claude/commands/a.md"), "ok\n").unwrap();
        std::fs::write(dir.0.join("docs/ref.md"), "rt-2026-01-01-gone\n")
            .unwrap();
        std::fs::write(dir.0.join("docs/issues/p.md"), "rt-2026-01-01-gone\n")
            .unwrap();
        let cfg = Config::for_project("p", dir.0.clone());
        let e = check(&cfg).unwrap_err();
        assert!(e.contains("docs/ref.md:1"), "{e}");
        assert!(!e.contains("docs/issues/p.md"), "exempt: {e}");
        std::fs::write(dir.0.join("docs/ref.md"), "fine\n").unwrap();
        assert_eq!(check(&cfg).unwrap(), "3 files, 5 checks");
    }

    #[test]
    fn globs_dirs_and_allowlisted_paths_are_skipped() {
        let none = |_: &str| false;
        for text in [
            "`docs/developer/*-log.md`",
            "`docs/issues/`",
            "`scripts/e2e.sh`",
            "`crates/rustbase/Cargo.toml`",
        ] {
            assert!(
                missing_paths("a.md", text, &none, &rules()).is_empty(),
                "{text} should be skipped"
            );
        }
    }

    #[test]
    fn non_repo_backticks_are_not_paths() {
        let none = |_: &str| false;
        let got =
            missing_paths("a.md", "`--dry-run` and `String`", &none, &rules());
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn ungranted_git_subcommand_is_reported() {
        let doc = "allowed-tools: Bash(git status:*)\n\nrun `git tag` now\n";
        let f = ungranted_git("c.md", doc);
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("git tag"));
    }

    #[test]
    fn a_quote_inside_the_command_does_not_hide_it() {
        let doc = "allowed-tools: Bash(git status:*)\n\n\
                   Run `git commit -m \"wip\"` now.\n";
        let f = ungranted_git("c.md", doc);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].message.contains("git commit"), "{f:?}");
    }

    #[test]
    fn a_narrow_grant_does_not_cover_a_wider_form() {
        let doc = "allowed-tools: Bash(git stash list:*)\n\n\
                   `git stash list` then `git stash drop`\n";
        let f = ungranted_git("c.md", doc);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].message.contains("git stash drop"), "{f:?}");
    }

    #[test]
    fn granted_subcommand_and_narrower_grant_both_pass() {
        let doc = "allowed-tools: Bash(git status:*), Bash(git config --get:*)\n\n`git status` and `git config --get user.email`\n";
        let got = ungranted_git("c.md", doc);
        assert!(got.is_empty(), "{got:?}");
    }

    /// Quotes are not guessed at: a command the skill only prints
    /// for the operator is declared, like any other ungranted one.
    #[test]
    fn a_printed_command_needs_the_declaration() {
        let printed = "allowed-tools: Bash(git log:*)\n\n\
                       - \"Push with `git push`\"\n";
        let f = ungranted_git("c.md", printed);
        assert_eq!(f.len(), 1, "{f:?}");
        let declared =
            format!("{printed}This skill has no `git push` grant.\n");
        let got = ungranted_git("c.md", &declared);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn a_whole_git_grant_covers_every_git_command() {
        for grant in ["Bash(git:*)", "Bash(git *)", "Bash", "Bash(*)"] {
            let doc = format!(
                "allowed-tools: Read, {grant}, Edit\n\n`git push -u origin x`\n"
            );
            let got = ungranted_git("c.md", &doc);
            assert!(got.is_empty(), "{grant}: {got:?}");
        }
    }

    #[test]
    fn each_command_in_a_chain_is_checked() {
        let doc = "allowed-tools: Bash(git status:*)\n\n\
                   `git status && git push`\n";
        let f = ungranted_git("c.md", doc);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].message.contains("git push"), "{f:?}");
    }

    /// The declaration is prose and wraps, so the match has
    /// to survive a newline landing inside it.
    #[test]
    fn a_declared_missing_grant_is_accepted_even_when_wrapped() {
        let doc = "allowed-tools: Bash(git log:*)\n\ntell them to run\n`git reset -- <path>` themselves; this skill has no\n`git reset` grant, deliberately.\n";
        let got = ungranted_git("c.md", doc);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn a_file_without_allowed_tools_is_not_checked_for_grants() {
        let got = ungranted_git("x.md", "run `git push`\n");
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn over_wide_prose_is_reported() {
        let long = "x".repeat(81);
        assert_eq!(over_wide("a.md", &long, 80).len(), 1);
        let got = over_wide("a.md", &"y".repeat(80), 80);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn frontmatter_tables_fences_and_indents_are_exempt() {
        let long = "x".repeat(120);
        let cases = [
            format!("---\ndescription: {long}\n---\n"),
            format!("| {long} |\n"),
            format!("```\n{long}\n```\n"),
            format!("    {long}\n"),
        ];
        for c in &cases {
            assert!(
                over_wide("a.md", c, 80).is_empty(),
                "should be exempt: {}",
                &c[..12.min(c.len())]
            );
        }
    }

    #[test]
    fn unknown_backlog_id_is_reported_and_known_one_is_not() {
        let known: BTreeSet<String> =
            ["rt-2026-09-03-real".to_string()].into_iter().collect();
        let got = unknown_ids("a.md", "see rt-2026-09-03-real", &known);
        assert!(got.is_empty(), "{got:?}");
        let f = unknown_ids("a.md", "see rt-2026-09-03-ghost", &known);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, "id");
    }

    #[test]
    fn id_shape_is_prefix_date_and_slug() {
        assert!(is_backlog_id("aq-2026-09-03-a-slug"));
        assert!(!is_backlog_id("rt-2026-09-03"));
        assert!(!is_backlog_id("rt-26-9-3-slug"));
        assert!(!is_backlog_id("xx-2026-09-03-slug"));
        assert!(!is_backlog_id("review"));
        assert!(!is_backlog_id("rt-2026-13-45-bad-date"));
    }

    #[test]
    fn backlog_headings_yield_their_ids() {
        let ids =
            ids_in_backlog("# Log\n\n### rt-2026-09-03-one\n\n### Not an id\n");
        assert_eq!(ids.len(), 1);
        assert!(ids.contains("rt-2026-09-03-one"));
    }

    #[test]
    fn backlog_headings_are_read_as_the_records_check_reads_them() {
        // A titled heading is an entry; one in a code block is not.
        let ids = ids_in_backlog(
            "### rt-2026-09-03-two -- A title\n\n```\n### rt-2026-09-03-x\n```\n",
        );
        assert_eq!(ids, BTreeSet::from(["rt-2026-09-03-two".to_owned()]));
    }

    #[test]
    fn record_and_working_files_are_exempt_from_the_citation_scan() {
        // Record files and issue docs cite IDs as data/provenance;
        // reference docs are read.
        assert!(is_citation_exempt(
            "docs/developer/template-feedback.md",
            &Rules::default()
        ));
        assert!(is_citation_exempt(
            "docs/developer/redteam-log.md",
            &Rules::default()
        ));
        assert!(is_citation_exempt(
            "docs/issues/anything.md",
            &Rules::default()
        ));
        assert!(!is_citation_exempt(
            "docs/architecture.md",
            &Rules::default()
        ));
        assert!(!is_citation_exempt(
            "docs/developer/supply-chain.md",
            &Rules::default()
        ));
    }

    #[test]
    fn a_dangling_citation_in_a_docs_file_is_reported() {
        // The scan is path-agnostic, so a docs/ file is checked the
        // same as a canon file once collect() feeds it in.
        let known = BTreeSet::new();
        let f = unknown_ids(
            "docs/architecture.md",
            "see rt-2026-09-03-ghost",
            &known,
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].file, "docs/architecture.md");
    }
}
