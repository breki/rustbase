//! Line-editing helpers for the markdown files the tooling writes
//! (`CHANGELOG.md`, `docs/todo.md`): splitting and rejoining lines
//! without changing their endings, finding a `## ` section by its
//! column-0 heading, and wrapping text without flattening its
//! structure.

/// True when `line` opens or closes a fenced code block (its first
/// non-whitespace run is ```` ``` ```` or `~~~`). `wrap_markdown`
/// uses it to copy fenced code through unwrapped; sections are found
/// by column-0 headings and do not track fences.
pub fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// The column width markdown is wrapped to: 80, the markdown width
/// `CLAUDE.md` sets and canon-check enforces. A literal, not read from
/// any config.
pub const MARKDOWN_WIDTH: usize = 80;

/// Split file content into owned lines, dropping the trailing
/// newline; [`rejoin_like`] restores it.
#[must_use]
pub fn to_owned_lines(content: &str) -> Vec<String> {
    content.lines().map(str::to_owned).collect()
}

/// Rejoin `lines` with the line ending and trailing-newline state of
/// `original`.
///
/// `str::lines()` drops a trailing `\r`, so rejoining with a bare
/// `\n` would rewrite every line of a CRLF file on a one-line edit.
/// The ending is taken from the *first* line break: a file with one
/// stray CRLF among LF lines stays LF.
#[must_use]
pub fn rejoin_like<S: AsRef<str>>(lines: &[S], original: &str) -> String {
    let sep = match original.find('\n') {
        Some(i) if original[..i].ends_with('\r') => "\r\n",
        _ => "\n",
    };
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push_str(sep);
        }
        out.push_str(line.as_ref());
    }
    if original.ends_with('\n') {
        out.push_str(sep);
    }
    out
}

/// Locate a `## <heading>` section: `(heading_index, section_end)`,
/// where `section_end` is the next `## ` heading or the line count,
/// so the body is `(heading_index + 1)..section_end`. `None` when the
/// heading is absent.
///
/// Both ends match at column 0 only (trailing whitespace ignored).
/// Correct for files whose entries are indented under a column-0
/// bullet, as `changelog` and `todo add` write them: nothing inside an
/// entry -- a heading-like line, a code fence, closed or not -- is
/// structure. A file with column-0 entry headings needs another finder. Code fences are deliberately not tracked: one that
/// looked unclosed would make the rest of the file read as code.
#[must_use]
pub fn section_bounds<S: AsRef<str>>(
    lines: &[S],
    heading: &str,
) -> Option<(usize, usize)> {
    let mut start = None;
    for (i, line) in lines.iter().enumerate() {
        let line = line.as_ref();
        match start {
            None if line.trim_end() == heading => start = Some(i),
            Some(h) if line.starts_with("## ") => return Some((h, i)),
            _ => {}
        }
    }
    start.map(|h| (h, lines.len()))
}

/// Reject empty or whitespace-only input, so a contentless `- ` stub
/// is never written; `label` names the field in the error.
///
/// # Errors
///
/// When `value` is empty or only whitespace.
pub fn require_nonempty(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(())
    }
}

/// A markdown list marker: its literal text and the indent it sits
/// at, both re-emitted verbatim, so `1. ` stays `1. ` and a nested
/// item stays nested.
struct Marker {
    indent: usize,
    text: String,
}

/// Whether `line` opens a markdown list item.
fn list_marker(line: &str) -> Option<Marker> {
    let t = line.trim_start();
    let indent = line.len() - t.len();
    for m in ["- ", "* ", "+ "] {
        if t.starts_with(m) {
            return Some(Marker {
                indent,
                text: m.to_owned(),
            });
        }
    }
    // Ordered markers: `1. `, `12. `. The digits are ASCII, so their
    // char count is also their byte count and the slice is safe.
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && t[digits..].starts_with(". ") {
        return Some(Marker {
            indent,
            text: t[..digits + 2].to_owned(),
        });
    }
    None
}

/// Wrap one paragraph's words.
fn wrap_block(
    words: &str,
    first: &str,
    cont: &str,
    width: usize,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::from(first);
    let mut has_word = false;
    for word in words.split_whitespace() {
        if has_word && cur.len() + 1 + word.len() > width {
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
    if has_word {
        lines.push(cur);
    }
    lines
}

/// What a block of the input is.
enum BlockKind {
    /// A paragraph or a list item (with its marker), reflowed.
    Prose {
        marker: Option<Marker>,
        words: String,
    },
    /// A fenced region, copied through line for line.
    Fenced(Vec<String>),
}

/// One structural unit of the input.
struct Block {
    /// Emit a blank line before this block.
    blank_before: bool,
    kind: BlockKind,
}

/// Split `text` into blocks: paragraphs, list items, and fenced
/// regions.
fn split_blocks(text: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut pending_blank = false;
    let mut fenced = false;

    for line in text.lines() {
        // A fenced region survives byte for byte: reflowing it
        // destroys the code and can leave an unbalanced fence, which
        // renders the rest of the file as code.
        if fenced || is_fence(line) {
            let line = line.trim_end().to_owned();
            if fenced {
                if let Some(Block {
                    kind: BlockKind::Fenced(lines),
                    ..
                }) = blocks.last_mut()
                {
                    lines.push(line.clone());
                }
                fenced = !is_fence(&line);
            } else {
                blocks.push(Block {
                    blank_before: pending_blank,
                    kind: BlockKind::Fenced(vec![line]),
                });
                pending_blank = false;
                fenced = true;
            }
            continue;
        }
        if line.trim().is_empty() {
            pending_blank = !blocks.is_empty();
            continue;
        }

        let marker = list_marker(line);
        if marker.is_none()
            && !pending_blank
            && let Some(Block {
                kind:
                    BlockKind::Prose {
                        marker: None,
                        words,
                    },
                ..
            }) = blocks.last_mut()
        {
            words.push(' ');
            words.push_str(line.trim());
            continue;
        }
        let t = line.trim_start();
        let words = marker.as_ref().map_or_else(
            || t.trim_end().to_owned(),
            |m| t[m.text.len()..].trim_end().to_owned(),
        );
        blocks.push(Block {
            blank_before: pending_blank,
            kind: BlockKind::Prose { marker, words },
        });
        pending_blank = false;
    }
    // An unclosed fence is closed with its own opening run, so the
    // written file never reads as code from here to its end.
    if fenced
        && let Some(Block {
            kind: BlockKind::Fenced(lines),
            ..
        }) = blocks.last_mut()
    {
        let opener = lines[0].trim_start();
        let mark = opener.chars().next().unwrap_or('`');
        let run: String = opener.chars().take_while(|&c| c == mark).collect();
        lines.push(run);
    }
    blocks
}

/// Wrap `text` to `width`, keeping its block structure.
///
/// Lines stay within `width` *bytes* (display columns for ASCII; a
/// multibyte char counts its UTF-8 length, so a line may wrap a
/// little early but never runs over). A single word longer than the
/// width overflows onto its own line rather than being split.
/// `first` prefixes the first emitted line, `cont` every other.
///
/// A blank line stays blank, a list item keeps its own marker and
/// indent, and a fenced region is copied verbatim. Blank input yields
/// no lines.
#[must_use]
pub fn wrap_markdown(
    text: &str,
    first: &str,
    cont: &str,
    width: usize,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, b) in split_blocks(text).iter().enumerate() {
        if b.blank_before {
            out.push(String::new());
        }
        // The first emitted line takes `first`, every other `cont`; a
        // list item appends its marker to whichever applies.
        let outer = if i == 0 { first } else { cont };
        match &b.kind {
            BlockKind::Fenced(lines) => {
                for (j, line) in lines.iter().enumerate() {
                    let prefix = if j == 0 { outer } else { cont };
                    out.push(format!("{prefix}{line}"));
                }
            }
            BlockKind::Prose { marker, words } => {
                let (block_first, block_cont) = match marker {
                    Some(m) => {
                        let pad = " ".repeat(m.indent);
                        (
                            format!("{outer}{pad}{}", m.text),
                            format!("{cont}{pad}{}", " ".repeat(m.text.len())),
                        )
                    }
                    None => (outer.to_owned(), cont.to_owned()),
                };
                out.extend(wrap_block(words, &block_first, &block_cont, width));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_fence_detects_both_markers() {
        assert!(is_fence("```"));
        assert!(is_fence("```rust"));
        assert!(is_fence("  ~~~"));
        assert!(!is_fence("### heading"));
        assert!(!is_fence("plain"));
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
    fn no_entry_text_can_end_or_extend_its_section() {
        // Entries are written indented, and only column-0 headings
        // are structure, so nothing in the text -- a fence first, an
        // unclosed fence, a word that wraps to look like a fence, a
        // heading-like line -- can move the section's end.
        for text in [
            "```sh\ncargo x\n```",
            "see\n```sh\nfoo",
            "a b c d ```foo``` e",
            "a b c d ~~~tilde e",
            "Example:\n```md\n### Notes\n## [0.0.1]\n```",
            "## not a heading",
        ] {
            let mut lines = wrap_markdown(text, "- ", "  ", 12);
            lines.insert(0, "## [Unreleased]".to_owned());
            lines.push("## [0.1.0]".to_owned());
            let n = lines.len();
            assert_eq!(
                section_bounds(&lines, "## [Unreleased]"),
                Some((0, n - 1)),
                "{lines:?}"
            );
        }
    }

    #[test]
    fn section_bounds_finds_a_heading_with_trailing_whitespace() {
        let lines = to_owned_lines("# T\n## [Unreleased]  \n- a\n## [0.1.0]\n");
        assert_eq!(section_bounds(&lines, "## [Unreleased]"), Some((1, 3)));
    }

    #[test]
    fn section_bounds_finds_body_range() {
        let lines = to_owned_lines("## A\n\n- one\n\n## B\n\n- two\n");
        assert_eq!(section_bounds(&lines, "## A"), Some((0, 4)));
        assert_eq!(section_bounds(&lines, "## B"), Some((4, lines.len())));
        assert_eq!(section_bounds(&lines, "## Missing"), None);
    }

    #[test]
    fn rejoin_like_round_trips_trailing_newline() {
        let with = "a\nb\n";
        assert_eq!(rejoin_like(&to_owned_lines(with), with), with);
        let without = "a\nb";
        assert_eq!(rejoin_like(&to_owned_lines(without), without), without);
    }

    #[test]
    fn wrap_markdown_returns_nothing_for_blank_input() {
        for blank in ["", "   \n  "] {
            let got = wrap_markdown(blank, "- ", "  ", 80);
            assert!(got.is_empty(), "{got:?}");
        }
    }

    #[test]
    fn wrap_markdown_keeps_paragraph_breaks() {
        let out = wrap_markdown("one two\n\nthree four", "- ", "  ", 80);
        assert_eq!(out, vec!["- one two", "", "  three four"]);
    }

    #[test]
    fn wrap_markdown_keeps_list_items_on_their_own_lines() {
        let text = "Decisions:\n- alpha choice\n- beta choice";
        let out = wrap_markdown(text, "- ", "  ", 80);
        assert_eq!(
            out,
            vec!["- Decisions:", "  - alpha choice", "  - beta choice"]
        );
    }

    #[test]
    fn wrap_markdown_wraps_a_long_list_item_under_its_marker() {
        // `first` is the body indent, as `todo add --body` passes
        // it; the item's own marker comes from the text.
        let text = "- alpha beta gamma delta";
        let out = wrap_markdown(text, "  ", "  ", 16);
        assert_eq!(out, vec!["  - alpha beta", "    gamma delta"]);
    }

    #[test]
    fn section_bounds_does_not_track_code_fences() {
        // An unclosed indented fence in an entry cannot make the rest
        // of the file look like code: the next column-0 heading still
        // ends the section.
        let lines = to_owned_lines("## A\n\n- see\n  ```sh\n  foo\n\n## B\n");
        assert_eq!(section_bounds(&lines, "## A"), Some((0, 6)));
    }

    #[test]
    fn section_bounds_ignores_an_indented_heading() {
        // `todo add` writes bodies at a 2-space indent, so an
        // indented `## ` is content, not structure. Treating it as
        // a terminator lets a body silently truncate its section.
        let lines = to_owned_lines("## A\n\n- one\n  ## B\n\n- two\n\n## C\n");
        let (_, end) = section_bounds(&lines, "## A").unwrap();
        assert_eq!(lines[end], "## C");
    }

    #[test]
    fn wrap_markdown_keeps_the_first_block_bullet() {
        // The first block keeps its list marker.
        let out = wrap_markdown("- one\n- two", "  ", "  ", 80);
        assert_eq!(out, vec!["  - one", "  - two"]);
    }

    #[test]
    fn wrap_markdown_preserves_marker_kind_and_nesting() {
        let out = wrap_markdown("1. one\n2. two", "  ", "  ", 80);
        assert_eq!(out, vec!["  1. one", "  2. two"]);

        let nested = wrap_markdown("- a\n  - b", "  ", "  ", 80);
        assert_eq!(nested, vec!["  - a", "    - b"]);
    }

    #[test]
    fn wrap_markdown_copies_fenced_regions_verbatim() {
        let text = "text:\n```sh\nsudo tailscale serve  --bg\n```\nafter";
        let out = wrap_markdown(text, "  ", "  ", 80);
        assert_eq!(
            out,
            vec![
                "  text:",
                "  ```sh",
                "  sudo tailscale serve  --bg",
                "  ```",
                "  after",
            ]
        );
        // Balanced: an odd count would render the rest of the file
        // as code.
        assert_eq!(out.iter().filter(|l| is_fence(l)).count() % 2, 0);
    }

    #[test]
    fn rejoin_like_does_not_convert_a_mixed_file_wholesale() {
        // One stray CRLF must not drag every other line with it.
        let mixed = "a\nb\r\nc\n";
        let out = rejoin_like(&to_owned_lines(mixed), mixed);
        assert!(!out.contains("\r\n"), "{out:?}");
    }

    #[test]
    fn rejoin_preserves_crlf_endings() {
        let crlf = "a\r\nb\r\n";
        let lines = to_owned_lines(crlf);
        assert_eq!(rejoin_like(&lines, crlf), crlf);
        let lf = "a\nb\n";
        assert_eq!(rejoin_like(&to_owned_lines(lf), lf), lf);
    }

    #[test]
    fn require_nonempty_accepts_content_rejects_blank() {
        assert!(require_nonempty("x", "hello").is_ok());
        let err = require_nonempty("--summary", "   ").unwrap_err();
        assert!(err.contains("--summary"));
        assert!(require_nonempty("x", "").is_err());
    }
}
