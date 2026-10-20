//! The `CHANGELOG.md` writer: insert a bullet into the right
//! subsection of `## [Unreleased]`, infer the `SemVer` bump the
//! accumulated bullets call for, and promote `[Unreleased]` to a
//! dated release section.
//!
//! Hand-editing is error-prone: `### Added` and `### Changed` can
//! sit dozens of lines apart, so a duplicate heading easily splits a
//! block. These functions own placement; the caller supplies the
//! content. Existing bullets are kept verbatim; only blank-line
//! spacing inside the one edited subsection is normalized.

use std::fmt;
use std::fs;

use crate::Config;
use crate::markdown::{
    MARKDOWN_WIDTH, rejoin_like, require_nonempty, section_bounds,
    to_owned_lines, wrap_markdown,
};
use crate::text::is_iso_date;

/// The heading every operation edits under.
const UNRELEASED: &str = "## [Unreleased]";

/// The fresh `[Unreleased]` block `promote` inserts: the heading
/// with empty subsections, ending in a blank line so it is
/// separated from the dated section below.
const UNRELEASED_SKELETON: [&str; 10] = [
    UNRELEASED,
    "",
    "### Added",
    "",
    "### Changed",
    "",
    "### Fixed",
    "",
    "### Removed",
    "",
];

/// A `[Unreleased]` subsection, in canonical order.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum Kind {
    /// New features.
    Added,
    /// Changes to existing behaviour.
    Changed,
    /// Bug fixes.
    Fixed,
    /// Removed functionality.
    Removed,
}

impl Kind {
    /// The subsections in the order they appear in the skeleton.
    const ORDER: [Kind; 4] =
        [Kind::Added, Kind::Changed, Kind::Fixed, Kind::Removed];

    /// The `### <label>` heading line for this kind.
    pub fn heading(self) -> &'static str {
        match self {
            Kind::Added => "### Added",
            Kind::Changed => "### Changed",
            Kind::Fixed => "### Fixed",
            Kind::Removed => "### Removed",
        }
    }

    /// Position in the canonical ordering (0-based).
    fn order(self) -> usize {
        Self::ORDER
            .iter()
            .position(|&k| k == self)
            .expect("every Kind is in ORDER")
    }

    /// The kind a `### <label>` heading line names, if any.
    fn from_heading(line: &str) -> Option<Self> {
        Self::ORDER.into_iter().find(|k| k.heading() == line.trim())
    }
}

/// A `SemVer` bump level. Displays as `major`, `minor` or `patch`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Bump {
    /// A `**BREAKING:**` bullet or a non-empty `### Removed`.
    Major,
    /// A non-empty `### Added`.
    Minor,
    /// Anything else.
    Patch,
}

impl fmt::Display for Bump {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Bump::Major => "major",
            Bump::Minor => "minor",
            Bump::Patch => "patch",
        })
    }
}

/// Adds `text` as a bullet under `kind` in the workspace's
/// `CHANGELOG.md`, prefixed `**BREAKING:**` when `breaking`.
///
/// # Errors
///
/// Blank text, or a `CHANGELOG.md` that cannot be read or written or
/// has no `## [Unreleased]` section.
pub fn add(
    cfg: &Config,
    kind: Kind,
    breaking: bool,
    text: &str,
) -> Result<(), String> {
    require_nonempty("changelog entry text", text)?;
    let bullet = if breaking {
        format!("**BREAKING:** {text}")
    } else {
        text.to_owned()
    };
    edit(cfg, |content| insert_bullet(content, kind, &bullet))
}

/// The bump the `[Unreleased]` bullets call for.
///
/// # Errors
///
/// An unreadable `CHANGELOG.md`, no `## [Unreleased]` section, or no
/// bullet in it (nothing to release).
pub fn infer_bump(cfg: &Config) -> Result<Bump, String> {
    bump_of(&read(cfg)?)
}

/// Renames `## [Unreleased]` to `## [version] - date` and inserts a
/// fresh empty `[Unreleased]` above it.
///
/// # Errors
///
/// A `version` that is not `X.Y.Z`, a `date` that is not
/// `YYYY-MM-DD`, nothing to release, or an unreadable or unwritable
/// `CHANGELOG.md`.
pub fn promote(cfg: &Config, version: &str, date: &str) -> Result<(), String> {
    edit(cfg, |content| promoted(content, version, date))
}

fn read(cfg: &Config) -> Result<String, String> {
    let path = cfg.root.join("CHANGELOG.md");
    fs::read_to_string(&path)
        .map_err(|e| format!("read {}: {e}", path.display()))
}

/// Read `CHANGELOG.md`, apply `change`, write the result back.
fn edit(
    cfg: &Config,
    change: impl FnOnce(&str) -> Result<String, String>,
) -> Result<(), String> {
    let updated = change(&read(cfg)?)?;
    let path = cfg.root.join("CHANGELOG.md");
    fs::write(&path, updated)
        .map_err(|e| format!("write {}: {e}", path.display()))
}

/// The `[Unreleased]` bounds, or the error every operation gives
/// without them.
fn unreleased(lines: &[String]) -> Result<(usize, usize), String> {
    section_bounds(lines, UNRELEASED)
        .ok_or_else(|| "CHANGELOG.md has no '## [Unreleased]' section".into())
}

/// Whether a line is a top-level markdown bullet (`- ...`).
fn is_bullet(line: &str) -> bool {
    line.trim_start().starts_with("- ")
}

/// The bump `content`'s `[Unreleased]` bullets call for.
fn bump_of(content: &str) -> Result<Bump, String> {
    let lines = to_owned_lines(content);
    let (u_start, u_end) = unreleased(&lines)?;
    let body = (u_start + 1)..u_end;
    if !body.clone().any(|i| is_bullet(&lines[i])) {
        return Err("nothing to release: [Unreleased] has no bullets".into());
    }
    let breaking = body
        .clone()
        .any(|i| lines[i].trim_start().starts_with("- **BREAKING:**"));
    let nonempty = |kind| subsection_nonempty(&lines, u_start, u_end, kind);
    Ok(if breaking || nonempty(Kind::Removed) {
        Bump::Major
    } else if nonempty(Kind::Added) {
        Bump::Minor
    } else {
        Bump::Patch
    })
}

/// Whether the `kind` subsection of `[Unreleased]` holds a bullet.
fn subsection_nonempty(
    lines: &[String],
    u_start: usize,
    u_end: usize,
    kind: Kind,
) -> bool {
    let Some(h) =
        ((u_start + 1)..u_end).find(|&i| lines[i].trim() == kind.heading())
    else {
        return false;
    };
    let block_end = subsection_block_end(lines, h + 1, u_end);
    ((h + 1)..block_end).any(|i| is_bullet(&lines[i]))
}

/// `content` with `[Unreleased]` promoted to `[version] - date`.
fn promoted(
    content: &str,
    version: &str,
    date: &str,
) -> Result<String, String> {
    let parts: Vec<&str> = version.split('.').collect();
    let is_semver = parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    if !is_semver {
        return Err(format!("version {version:?} is not X.Y.Z"));
    }
    if !is_iso_date(date) {
        return Err(format!("date {date:?} is not YYYY-MM-DD"));
    }
    bump_of(content)?;
    let mut lines = to_owned_lines(content);
    let (u_start, _) = unreleased(&lines)?;
    lines[u_start] = format!("## [{version}] - {date}");
    let skeleton = UNRELEASED_SKELETON.iter().map(|&s| s.to_owned());
    lines.splice(u_start..u_start, skeleton);
    Ok(rejoin_like(&lines, content))
}

/// The end (exclusive) of a `### ` subsection whose body starts
/// at `start`: the next `### ` heading within `(start..u_end)`,
/// or `u_end`.
fn subsection_block_end(lines: &[String], start: usize, u_end: usize) -> usize {
    (start..u_end)
        .find(|&i| lines[i].trim_start().starts_with("### "))
        .unwrap_or(u_end)
}

/// Insert `text` as a bullet under `kind` in the `[Unreleased]`
/// section, returning the new file content. Pure -- no I/O -- so
/// it is unit-testable against fixtures.
fn insert_bullet(
    content: &str,
    kind: Kind,
    text: &str,
) -> Result<String, String> {
    let mut lines = to_owned_lines(content);

    let (u_start, u_end) = unreleased(&lines)?;

    let bullet = wrap_markdown(text, "- ", "  ", MARKDOWN_WIDTH);

    if let Some(h) =
        ((u_start + 1)..u_end).find(|&i| lines[i].trim() == kind.heading())
    {
        append_to_existing_block(&mut lines, h, u_end, bullet);
    } else {
        insert_new_block(&mut lines, u_start, u_end, kind, bullet);
    }

    Ok(rejoin_like(&lines, content))
}

/// Append the bullet to an existing `### <kind>` block whose
/// heading is at `h`. Existing bullet lines are preserved; only
/// the block's blank-line spacing is normalized (one leading and
/// one trailing blank around the bullets).
fn append_to_existing_block(
    lines: &mut Vec<String>,
    h: usize,
    u_end: usize,
    bullet: Vec<String>,
) {
    let block_end = subsection_block_end(lines, h + 1, u_end);
    // Insert after the last non-blank line so existing bullets --
    // including any interior blank lines of a multi-paragraph
    // entry -- survive verbatim. Only an empty block (blank lines
    // only) is rewritten, to give canonical heading/blank/bullet/
    // blank spacing.
    if let Some(last) = ((h + 1)..block_end)
        .rev()
        .find(|&i| !lines[i].trim().is_empty())
    {
        let at = last + 1;
        lines.splice(at..at, bullet);
    } else {
        let mut new_block = vec![String::new()];
        new_block.extend(bullet);
        new_block.push(String::new());
        lines.splice((h + 1)..block_end, new_block);
    }
}

/// Create a new `### <kind>` block in canonical order (used only
/// when the skeleton is missing that heading) and put the bullet
/// in it.
fn insert_new_block(
    lines: &mut Vec<String>,
    u_start: usize,
    u_end: usize,
    kind: Kind,
    bullet: Vec<String>,
) {
    // Insert before the first existing heading whose canonical
    // order is greater than this kind's; else at the section end.
    let at = ((u_start + 1)..u_end)
        .find(|&i| {
            Kind::from_heading(&lines[i])
                .is_some_and(|k| k.order() > kind.order())
        })
        .unwrap_or(u_end);
    // A heading needs a blank line above it; at the section end
    // the line above can be the previous block's last bullet.
    let mut block = Vec::new();
    if at > 0 && !lines[at - 1].trim().is_empty() {
        block.push(String::new());
    }
    block.push(kind.heading().to_owned());
    block.push(String::new());
    block.extend(bullet);
    block.push(String::new());
    lines.splice(at..at, block);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKELETON: &str = "\
# Changelog

## [Unreleased]

### Added

### Changed

### Fixed

### Removed

## [0.1.0] - 2026-01-01

### Added

- First release.
";

    #[test]
    fn adds_to_empty_subsection() {
        let out =
            insert_bullet(SKELETON, Kind::Changed, "did a thing").unwrap();
        assert!(out.contains("### Changed\n\n- did a thing\n\n### Fixed"));
        // The released section is untouched.
        assert!(out.contains(
            "## [0.1.0] - 2026-01-01\n\n### Added\n\n- First release.\n"
        ));
    }

    #[test]
    fn appends_after_an_existing_bullet() {
        let start = "\
## [Unreleased]

### Added

- existing entry

### Changed
";
        let out = insert_bullet(start, Kind::Added, "second entry").unwrap();
        assert!(out.contains(
            "### Added\n\n- existing entry\n- second entry\n\n### Changed"
        ));
    }

    #[test]
    fn does_not_touch_a_released_section_with_the_same_kind() {
        // Adding to [Unreleased] Added must not land in [0.1.0].
        let out = insert_bullet(SKELETON, Kind::Added, "new feature").unwrap();
        let unreleased_added = out.find("### Added\n\n- new feature").unwrap();
        let released = out.find("## [0.1.0]").unwrap();
        assert!(unreleased_added < released);
    }

    #[test]
    fn breaking_prefix_is_applied_by_caller_path() {
        // insert_bullet takes the already-prefixed text.
        let out = insert_bullet(
            SKELETON,
            Kind::Removed,
            "**BREAKING:** dropped the old API",
        )
        .unwrap();
        assert!(out.contains("- **BREAKING:** dropped the old API"));
    }

    #[test]
    fn creates_a_missing_heading_in_canonical_order() {
        let start = "\
## [Unreleased]

### Added

- a feature

### Removed

- a removal
";
        // Fixed is missing; it must land between Added and Removed.
        let out = insert_bullet(start, Kind::Fixed, "a fix").unwrap();
        let added = out.find("### Added").unwrap();
        let fixed = out.find("### Fixed").unwrap();
        let removed = out.find("### Removed").unwrap();
        assert!(added < fixed && fixed < removed);
        assert!(out.contains("### Fixed\n\n- a fix\n"));
    }

    #[test]
    fn a_new_heading_at_the_section_end_keeps_its_blank_line() {
        // `[Unreleased]` ends with a bullet, not a blank (here the
        // file ends there), so a heading inserted at the section
        // end must bring its own blank line.
        let start = "\
# Changelog

## [Unreleased]

### Changed

- an existing entry
";
        let out = insert_bullet(start, Kind::Fixed, "a fix").unwrap();
        assert!(
            out.contains("- an existing entry\n\n### Fixed\n\n- a fix\n"),
            "{out}"
        );
    }

    #[test]
    fn preserves_interior_blank_lines_of_existing_bullets() {
        // A multi-paragraph existing bullet must keep its interior
        // blank line when a new bullet is appended to the block.
        let start = "\
## [Unreleased]

### Changed

- first entry

  second paragraph of first entry

### Fixed
";
        let out = insert_bullet(start, Kind::Changed, "new entry").unwrap();
        assert!(out.contains(
            "- first entry\n\n  second paragraph of first entry\n- new entry"
        ));
    }

    #[test]
    fn errors_without_an_unreleased_section() {
        let err = insert_bullet(
            "# Changelog\n\n## [0.1.0] - 2026-01-01\n",
            Kind::Added,
            "x",
        )
        .unwrap_err();
        assert!(err.contains("Unreleased"));
    }

    #[test]
    fn preserves_trailing_newline() {
        assert!(
            insert_bullet(SKELETON, Kind::Added, "x")
                .unwrap()
                .ends_with('\n')
        );
    }

    #[test]
    fn add_rejects_blank_entry_text_before_any_io() {
        // The guard fires before `CHANGELOG.md` is read, so this
        // never touches the real file.
        let cfg = Config::for_project("p", "/nonexistent".into());
        let err = add(&cfg, Kind::Added, false, "   ").unwrap_err();
        assert!(err.contains("empty"));
    }

    fn with(kind: Kind, text: &str) -> String {
        insert_bullet(SKELETON, kind, text).unwrap()
    }

    #[test]
    fn the_bump_follows_the_heaviest_entry() {
        assert_eq!(bump_of(&with(Kind::Fixed, "a fix")), Ok(Bump::Patch));
        assert_eq!(bump_of(&with(Kind::Added, "a feature")), Ok(Bump::Minor));
        assert_eq!(bump_of(&with(Kind::Removed, "a flag")), Ok(Bump::Major));
        let breaking = with(Kind::Changed, "**BREAKING:** a default");
        assert_eq!(bump_of(&breaking), Ok(Bump::Major));
        assert_eq!(Bump::Minor.to_string(), "minor");
    }

    #[test]
    fn there_is_no_bump_and_no_release_without_a_bullet() {
        assert!(
            bump_of(SKELETON)
                .unwrap_err()
                .contains("nothing to release")
        );
        let e = promoted(SKELETON, "0.2.0", "2026-10-20").unwrap_err();
        assert!(e.contains("nothing to release"), "{e}");
    }

    #[test]
    fn promote_dates_the_section_and_opens_a_fresh_one_above() {
        let before = with(Kind::Fixed, "a fix");
        let after = promoted(&before, "0.2.0", "2026-10-20").unwrap();
        let fresh = after.find("## [Unreleased]").unwrap();
        let dated = after.find("## [0.2.0] - 2026-10-20").unwrap();
        assert!(fresh < dated, "{after}");
        assert!(after[dated..].contains("- a fix"));
        assert!(!after[fresh..dated].contains("- a fix"));
        // The fresh block has no bullets, so nothing is releasable.
        assert!(bump_of(&after[..dated]).is_err());
    }

    #[test]
    fn promote_rejects_a_malformed_version_or_date() {
        let before = with(Kind::Fixed, "a fix");
        for v in ["0.2", "v0.2.0", "0.2.x", "0..1"] {
            assert!(promoted(&before, v, "2026-10-20").is_err(), "{v}");
        }
        assert!(promoted(&before, "0.2.0", "2026/10/20").is_err());
    }

    #[test]
    fn the_file_operations_read_and_write_changelog_md_in_the_root() {
        let dir = crate::test_repo::empty_dir();
        std::fs::write(dir.0.join("CHANGELOG.md"), SKELETON).unwrap();
        let cfg = Config::for_project("p", dir.0.clone());
        add(&cfg, Kind::Added, false, "a feature").unwrap();
        assert_eq!(infer_bump(&cfg), Ok(Bump::Minor));
        promote(&cfg, "0.2.0", "2026-10-20").unwrap();
        let text = std::fs::read_to_string(dir.0.join("CHANGELOG.md")).unwrap();
        assert!(text.contains("## [0.2.0] - 2026-10-20"), "{text}");
        let missing = Config::for_project("p", dir.0.join("nope"));
        assert!(infer_bump(&missing).unwrap_err().contains("read"));
    }

    #[test]
    fn wraps_a_long_entry_to_the_markdown_width() {
        let long = "this is a fairly long changelog entry that should be \
                    wrapped across more than one line by the markdown wrapper";
        let out = insert_bullet(SKELETON, Kind::Added, long).unwrap();
        // The entry landed under Added and no line exceeds the width.
        assert!(out.contains("### Added\n\n- this is a fairly"));
        assert!(out.lines().all(|l| l.len() <= MARKDOWN_WIDTH));
    }
}
