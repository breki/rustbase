//! Line-editing helpers for the markdown files the tooling writes
//! (`CHANGELOG.md`, `docs/todo.md`): splitting and rejoining lines
//! without changing their endings, finding a `## ` section outside
//! code fences, and wrapping text without flattening its structure.

/// True when `line` opens or closes a fenced code block (its
/// first non-whitespace run is ```` ``` ```` or `~~~`). Callers
/// toggle fence state on each such line so markdown headers
/// *inside* a code fence are not mistaken for structure.
pub fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// 80-column rule in `rustfmt.toml`). Shared by the `changelog`
/// and `todo` bullet writers.
pub const MARKDOWN_WIDTH: usize = 80;

/// Split file content into owned lines. Drops the trailing
/// newline; pair with [`rejoin_like`] to restore it. Shared by
/// the `changelog` and `todo` line-editing commands.
#[must_use]
pub fn to_owned_lines(content: &str) -> Vec<String> {
    content.lines().map(str::to_owned).collect()
}

/// Rejoin `lines` using the line ending and trailing-newline
/// state of `original`.
///
/// `to_owned_lines` goes through `str::lines()`, which drops a
/// trailing `\r`. Rejoining with a bare `\n` would therefore
/// rewrite every line of a CRLF file on a one-line edit --
/// burying the real change in whole-file churn on the Windows
/// checkouts this project supports.
///
/// The ending is taken from the *first* line break, not from
/// "contains a CRLF anywhere": a file with one stray CRLF among
/// LF lines would otherwise be converted wholesale to CRLF --
/// the same whole-file churn this exists to avoid, in the other
/// direction.
#[must_use]
pub fn rejoin_like(lines: &[String], original: &str) -> String {
    let sep = match original.find('\n') {
        Some(i) if original[..i].ends_with('\r') => "\r\n",
        _ => "\n",
    };
    let mut out = lines.join(sep);
    if original.ends_with('\n') {
        out.push_str(sep);
    }
    out
}

/// Locate a `## <heading>` section. Returns
/// `(heading_index, section_end)` where `section_end` is the
/// index of the next `## ` heading (or the line count) -- so the
/// section body is `(heading_index + 1)..section_end`. `None`
/// when the heading is absent. Shared by the `changelog` and
/// `todo` commands.
#[must_use]
pub fn section_bounds(
    lines: &[String],
    heading: &str,
) -> Option<(usize, usize)> {
    // Fence state is tracked from the top of the file so that a
    // `## ...` line inside a code block is never mistaken for
    // structure -- callers splice against these bounds, so a
    // wrong end index deletes a truncated range rather than
    // merely mis-reading one.
    let mut fenced = false;
    let mut start = None;
    for (i, line) in lines.iter().enumerate() {
        if is_fence(line) {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        match start {
            None if line == heading => start = Some(i),
            // Both ends are matched at column 0. Trimming the
            // terminator would make indented *content* structural
            // -- and `todo add` writes bodies at a 2-space indent,
            // so a body line beginning `## ` would silently end
            // the section it lives in.
            Some(_) if line.starts_with("## ") => {
                return Some((start?, i));
            }
            _ => {}
        }
    }
    start.map(|h| (h, lines.len()))
}

/// Reject empty / whitespace-only user input for the mechanical
/// markdown writers, so a contentless `- ` (or `- **slug** -- `)
/// stub is never written into `CHANGELOG.md` / `docs/todo.md`.
/// `label` names the offending field in the error.
///
/// # Errors
///
/// Returns an error when `value` is empty or only whitespace.
pub fn require_nonempty(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(())
    }
}

/// A markdown list marker: its literal text and the indent it
/// sits at.
///
/// Both are kept because the writer re-emits them verbatim.
/// Storing only a width collapses `1. ` to `- ` and flattens a
/// nested item to top level -- rewriting content the caller
/// supplied rather than merely placing it.
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
    // Ordered markers: `1. `, `12. `, ...
    // `take_while(is_ascii_digit)` only ever counts ASCII, so the
    // char count is also the byte count and the slice is safe.
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && t[digits..].starts_with(". ") {
        return Some(Marker {
            indent,
            text: t[..digits + 2].to_owned(),
        });
    }
    None
}

/// Wrap one already-split block of words.
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

/// One structural unit of the input.
struct Block {
    /// `Some` for a list item, carrying its marker verbatim.
    marker: Option<Marker>,
    /// Emit a blank line before this block.
    blank_before: bool,
    /// `true` for a fenced region, which is copied through
    /// unwrapped rather than reflowed.
    verbatim: bool,
    /// Wrappable text, or the literal lines when `verbatim`.
    lines: Vec<String>,
}

/// Split `text` into blocks: paragraphs, list items, and
/// fenced regions.
fn split_blocks(text: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut pending_blank = false;
    let mut fenced = false;

    for line in text.lines() {
        // A fenced region is structural: its lines must survive
        // byte-for-byte, and reflowing one both destroys the code
        // and can emit an unbalanced fence, which `section_bounds`
        // then honours for the rest of the file.
        if is_fence(line) {
            if fenced {
                if let Some(b) = blocks.last_mut() {
                    b.lines.push(line.trim_end().to_owned());
                }
                fenced = false;
            } else {
                blocks.push(Block {
                    marker: None,
                    blank_before: pending_blank,
                    verbatim: true,
                    lines: vec![line.trim_end().to_owned()],
                });
                pending_blank = false;
                fenced = true;
            }
            continue;
        }
        if fenced {
            if let Some(b) = blocks.last_mut() {
                b.lines.push(line.trim_end().to_owned());
            }
            continue;
        }
        if line.trim().is_empty() {
            pending_blank = !blocks.is_empty();
            continue;
        }

        let marker = list_marker(line);
        let continues = marker.is_none()
            && !pending_blank
            && blocks
                .last()
                .is_some_and(|b| b.marker.is_none() && !b.verbatim);
        if continues {
            let last = blocks.last_mut().expect("checked by `continues`");
            last.lines[0].push(' ');
            last.lines[0].push_str(line.trim());
        } else {
            let t = line.trim_start();
            let words = marker.as_ref().map_or_else(
                || t.trim_end().to_owned(),
                |m| t[m.text.len()..].trim_end().to_owned(),
            );
            blocks.push(Block {
                marker,
                blank_before: pending_blank,
                verbatim: false,
                lines: vec![words],
            });
            pending_blank = false;
        }
    }
    blocks
}

/// Wrap `text` to `width`, preserving its block structure.
///
/// Lines stay within `width` *bytes* (equal to display columns
/// for the ASCII-leaning markdown this project writes; a
/// multibyte char counts as its UTF-8 byte length, so a line
/// never *exceeds* `width` columns but may wrap a little
/// early). A single word longer than the available width
/// overflows onto its own line rather than being split
/// mid-word.
///
/// `first` prefixes the very first emitted line, `cont` every
/// other line -- the project's 80-column markdown with a
/// 2-space hanging indent. Shared by the `changelog` and `todo`
/// writers.
///
/// Structure survives the round trip: a blank line stays a
/// blank line, a list item keeps its own marker and indent
/// (`1.` is not rewritten as `-`, and a nested item stays
/// nested), and a fenced region is copied through verbatim.
/// Treating the whole body as one paragraph -- the obvious
/// implementation -- inlines a bullet list into prose and
/// collapses code, and the writers calling this have no way to
/// undo either afterwards.
///
/// Blank or whitespace-only input yields no lines at all, so
/// an empty `--body` cannot append a whitespace-only line.
#[must_use]
pub fn wrap_markdown(
    text: &str,
    first: &str,
    cont: &str,
    width: usize,
) -> Vec<String> {
    let blocks = split_blocks(text);
    let mut out: Vec<String> = Vec::new();

    for (i, b) in blocks.iter().enumerate() {
        if b.blank_before {
            out.push(String::new());
        }
        // The first emitted line takes `first`; every other line
        // takes `cont`. A list item appends its own marker to
        // whichever applies, so block 0 keeps its bullet.
        let outer = if i == 0 { first } else { cont };
        if b.verbatim {
            for (j, line) in b.lines.iter().enumerate() {
                let prefix = if i == 0 && j == 0 { first } else { cont };
                out.push(format!("{prefix}{line}"));
            }
            continue;
        }
        let (block_first, block_cont) = match &b.marker {
            Some(m) => {
                let pad = " ".repeat(m.indent);
                (
                    format!("{outer}{pad}{}", m.text),
                    format!("{cont}{pad}{}", " ".repeat(m.text.len())),
                )
            }
            None => (outer.to_owned(), cont.to_owned()),
        };
        out.extend(wrap_block(&b.lines[0], &block_first, &block_cont, width));
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
    fn section_bounds_ignores_headings_inside_a_code_fence() {
        let lines = to_owned_lines(
            "## A\n\n```\n## B\n```\n\n- one\n\n## B\n\n- two\n",
        );
        // The fenced `## B` must not terminate section A.
        let (start, end) = section_bounds(&lines, "## A").unwrap();
        assert_eq!(start, 0);
        assert_eq!(lines[end], "## B");
        assert!(end > 6, "section A ended early at {end}");
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
        // Block 0 used to lose its marker entirely.
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
        // Balanced: an odd count would flip `section_bounds` for
        // the rest of the file.
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
