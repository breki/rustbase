# Red Team Findings -- Deferred backlog

Red Team findings that were **deferred** -- real, but not
fixed at review time. Fixed findings are not logged here;
their resolution lives in the commit that fixed them.

Newest first; add new entries right after the `---`. Use a
self-describing ID `rt-<YYYY-MM-DD>-<kebab-slug>` (no central
counter); a later commit acting on an item cites the ID
inline. Each entry: a `### <id>` heading, a `**Category:**` line,
and a short description.

**Threshold:** when 10+ items are open here, a full-codebase
red team review is warranted before continuing feature work.

---

### rt-2026-10-20-scripts-check-one-git-per-file

**Category:** cost / offline

The scripts gate (`crates/rustbase-xtask/src/scripts.rs`) starts one
`git cat-file blob <sha>` per regular file under `scripts/` and reads
each blob whole to test its first two bytes. Negligible for a handful
of scripts; slow on Windows with many files, and in a blobless partial
clone whose sparse checkout leaves `scripts/` out each blob is fetched
lazily, so offline the gate fails. The fix: one `git cat-file --batch`
fed every sha, which needs stdin support in `process` and byte-level
parsing of its output. Deferred by the operator: no realistic project
here has more than a few scripts.

### rt-2026-10-20-wrap-fence-closing-not-commonmark

**Category:** output (rendering)

`split_blocks` in `crates/rustbase-xtask/src/markdown.rs` closes a fence
on any fence line, and its synthetic closer for an unclosed fence is
written at the entry's indent. CommonMark closes only on the same
character with a run at least as long, and reads an opener indented four
or more columns past the list content as indented code. Entry text such
as "x:\n```\n~~~\nfoo\n```" therefore renders as code to the end of the
file on GitHub, though the tooling reads the file correctly (sections
are found by column-0 headings). Track the opener's character and run
length. Found by the review of the changelog extraction (#7).

### rt-2026-10-20-check-counts-cargo-errors-as-compile-errors

**Category:** output (misleading summary)

`extract_error_lines` in `crates/rustbase-xtask/src/check.rs` keeps
every line starting `error:`, so cargo's own errors -- a bad manifest
(`error: string values must be quoted` then ` --> Cargo.toml:3:9`), an
unresolvable dependency -- are reported as `1 compilation error(s)`,
with the location and context lines dropped. Count only rustc
diagnostics (a `path:line:col: error` prefix or `error[E`) and send
the rest to the `Failure::of_run` tail. Pre-existing; found by the
review of the parser extraction (#7).

### rt-2026-10-19-canon-rules-can-switch-checks-off

**Category:** correctness (configuration)

`canon::Rules::validate` in `crates/rustbase-xtask/src/canon.rs`
refuses exemptions that would empty a check, but not
`repo_prefixes: vec![]` (or prefixes no canon path uses), which
silences the path check, nor an absurd `max_cols`, which silences the
width check; the Canon step stays green. Refuse an empty prefix list
and bound `max_cols` (e.g. 1..=200). Deferred: found in the last
allowed red-team round of the doc/canon extraction (#6).

### rt-2026-10-19-coverage-failure-paths-ambiguous

**Category:** output (failure message)

The coverage gate names failing files by the part of the path after
`src/` (`shorten_path` in `crates/rustbase-xtask/src/coverage.rs`), so
two crates' `lib.rs` read the same in one failure list and the
operator cannot tell which crate to fix. Name the crate too, e.g. a
workspace-relative path. Pre-existing; raised by the fresh-reader
stage of the coverage/dupes extraction (#6).

### rt-2026-10-19-release-deploy-existing-tag

**Category:** Correctness (procedure)

`/release` says a no-change ship can run `cargo xtask deploy`
"on the existing tag", but the deploy guard requires `HEAD` to
be on the tag, and `HEAD` has normally moved past it. Say to
check out the tag first, or drop the shortcut. Raised as a
possible bug by fresh-reader during /review; not verified.

### rt-2026-10-19-canon-width-skips-indented-prose

**Category:** Correctness (lenient gate)

`over_wide` in `crates/rustbase-xtask/src/canon.rs` exempts every
line that starts with four spaces, meant for indented code. Indented
list-continuation prose (most numbered steps in
`.claude/commands/`) starts the same way, so its width is never
checked. Distinguish a code block (blank line before, not inside
a list item) from a continuation, or check indented lines too.

### rt-2026-10-19-canon-git-check-gaps

**Category:** Correctness (lenient gate)

`ungranted_git` in `crates/rustbase-xtask/src/canon.rs` reports
nothing (a false green) for: a `git` line inside a fenced code block; an inline
span that wraps across lines (and a wrapped closing backtick
flips the pairing for the rest of its line); `git` that is not
the first word of a segment (`$(git ...)`, `GIT_EDITOR=true
git ...`, after `env`/`xargs`); and a wildcard-less grant such as
`Bash(git status)`, which the harness matches exactly but the
check reads as a prefix. All latent -- none hides an ungranted
command in today's canon. Found in /review round 3 (the ceiling)
after two redesign rounds on this check, so not fixed in that
run. Fix: scan fences as commands, pair backticks over
whitespace-collapsed paragraph text mapped back via
`Block::line_at`, split on `$(`/`(`/`)` and strip `NAME=value`
words, and record whether a grant had a wildcard.

### rt-2026-07-16-deploy-guard-toctou

**Category:** Security (TOCTOU)

`deploy_guard::require_release_tag` checks
`working_tree_clean` at the top of `deploy()`, but the tree
is not re-read until `sync_source` tars the sources and
`build_frontend` builds moments later. A change to the tree
in that window ships un-verified content under a tag that
vouches for the clean state. Low severity: operator-only
threat model (self-inflicted race, not an adversary), and the
binary is rebuilt from the tarred sources, not a stale
artifact. If tightening is wanted, re-assert
`working_tree_clean` immediately before `sync_source`, or tar
from `git archive <tag>` so the shipped source is provably the
tagged commit.
