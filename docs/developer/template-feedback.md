# Template feedback

Issues, improvements, and observations about the
[rustbase](https://github.com/breki/rustbase) template.

This file uses three lifecycle sections, the same shape
adopted by Ledgerstone (a downstream project) and now
shipped with the template itself:

- **Open divergences** -- things the project knows are
  suboptimal, missing, or differently-shaped than the
  ideal template baseline. In a derived project these
  are intentional or pending differences from the
  template; in this template repo they are known
  template issues awaiting fix.
- **Resolved** -- entries closed out by a retrofit /
  fix commit. Keeps the history visible without
  cluttering the open list.
- **Suggestions to flow back to the template** -- in a
  derived project, this is where ideas live that the
  project wants to push upstream. In this template repo
  the section is informational (there is no upstream),
  but the structure is preserved so new entries route
  identically across template and derived projects.

`/template-improve` adds new entries by asking which
section they belong to.

---

## Open divergences

### tf-2026-05-17-build-ps1-build-command-double-validates -- `build.ps1 build` command double-validates

`Invoke-Build` (line 36) calls `Invoke-Validate`, which
runs `cargo xtask validate` (fmt + clippy + test +
coverage). It then calls `Invoke-BuildOnly`, which runs
`cargo build --release`. But `cargo xtask validate`
already compiled the entire workspace (in debug mode)
as part of running clippy and tests.

The result: a full `.\build.ps1 build` compiles the
workspace twice -- once in debug for validate, once in
release for the final binary. This is expected if the
intent is "check everything, then produce a release
binary." But if a user just wants a validated release
build, there is no single command that avoids the
double-compile.

**Suggestion:** Document that `build` intentionally
compiles twice (validate in debug, then release), so
users understand the cost. Alternatively, consider a
`build-release` command that runs clippy and tests
against the release profile directly, avoiding the
redundant debug build.

### tf-2026-05-17-playwright-fixture-isolation-pattern-not-shipped -- Playwright fixture-isolation pattern not shipped

`rustbase-web` has no persistent data -- `/api/status`
and `/api/greeting` return static values -- so a
fixture-isolation example doesn't fit the current
sample app. The pattern becomes relevant the moment a
downstream project adds a data store.

**Suggestion (for when a data-persistence example is
eventually added to the template):** include the
fixture-isolation pattern alongside it (globalSetup
copies `e2e/fixtures/*.json` to `test-data/`, backend
run with `--data test-data`). Until then, leave it to
individual projects.

---

## Resolved

### tf-2026-10-19-changelog-add-glued-a-heading-to-a-bullet -- changelog add glued a heading to a bullet

Surfaced from bombyx's template feedback (2026-10-19). `changelog add`
inserted a new `### <kind>` heading with no blank line above it when
the insertion point followed a bullet, gluing the heading to that
bullet so it rendered as body text. The trigger is a section end with
no blank line before it, e.g. `[Unreleased]` ending the file (bombyx's
own test shape did not reproduce it here). `insert_new_block` now
pushes a blank line first when the line above is not blank, with a
regression test proven red first.

### tf-2026-10-19-commit-found-the-latest-tag-alphabetically -- commit found the latest tag alphabetically

Surfaced from bombyx's template feedback (2026-10-19). `/commit`'s
"did this ship?" check said to run `git tag` and take the latest tag,
but `git tag` sorts alphabetically, so `v0.9.0` read as newer than
`v0.11.0` and `<latest-tag>..HEAD` could cover shipped changes. It now
uses `git tag --list --sort=-v:refname` (first line is the latest) and
grants that command in `allowed-tools`, which it previously lacked.

### tf-2026-10-19-release-linted-the-host-target-on-linux -- release linted the host target on Linux

Surfaced from bombyx's template feedback (2026-10-19). `/release`'s
cross-platform clippy hardcoded `x86_64-unknown-linux-gnu`, which on a
Linux host is the host itself, so it repeated `validate` and checked
no other platform. It now lints every installed target other than the
host (`rustc -vV` gives the host), says to add a target rather than
skip, grants the two commands it needs, and the release summary names
the targets that ran.

### tf-2026-10-19-new-clippy-lint-broke-xtask-tests -- new clippy lint broke xtask tests

Surfaced from bombyx's template feedback (2026-10-19). Rust 1.99's
`assert_is_empty` lint rejected `assert!(x.is_empty())` and its
negated form, which the template used 23 times, so the unpinned
`stable` channel turned the Clippy gate red with no project change.
Every site now names the value in the failure message (or binds a
call result first) or uses `assert_ne!(.., "")`; verified with
`cargo +1.99.0 clippy --workspace --all-targets -D warnings`.
`CLAUDE.md` now says the channel is unpinned and what that costs.

### tf-2026-10-19-todo-md-was-missing-from-never-sync -- todo.md was missing from never-sync

Surfaced from bombyx's template feedback (2026-09-11). `docs/todo.md`
is a per-project backlog written by `cargo xtask todo`, but was not in
`sync-candidates`' `NEVER_SYNC`, so every sync would offer the
template's own backlog to downstreams. Added it (and the new
`fresh-reader-log.md`), asserted in `is_excluded_matches_exact_and_prefix`,
and updated `/template-sync`'s description of the set.

### tf-2026-10-19-template-shipped-no-doc-link-gate -- template shipped no doc-link gate

Surfaced from bombyx's template feedback (2026-10-19). Nothing gated
documentation: rustdoc reports a broken intra-doc link as a warning,
so docs built clean and stopped navigating. Ported `cargo xtask doc`,
which runs rustdoc with `-D warnings` twice (public, then
`--document-private-items`, since each pass sees a class of broken
link the other cannot), wired into `validate` after Clippy and into
the Stop hook (measured 1.2 s warm). It immediately caught an
ambiguous `[test]` link in `test_cmd.rs`. `CLAUDE.md` gained the rule
to reference private items by backticked name.

### tf-2026-10-19-xtask-invocations-in-command-files-were-not-quie -- xtask invocations in command files were not quiet

Surfaced from bombyx's template feedback (2026-08-30). Command files
wrote bare `cargo xtask ...`, so the first call after any xtask change
put dozens of `Compiling` lines ahead of the result. Invocations the
agent is told to run now use `cargo -q xtask` (prose that merely names
a command is unchanged), and every `allowed-tools` pattern for
`cargo xtask` gained a `cargo -q xtask` twin so the quiet spelling is
still granted. `CLAUDE.md` Build Commands documents the convention.

### tf-2026-10-19-ask-command-shipped -- ask command shipped

Surfaced from bombyx's template feedback (2026-10-19). Ported `/ask`,
which re-puts the last decision the assistant raised in prose as a
structured `AskUserQuestion` (recommendation first, no new options or
analysis), and listed it in the `CLAUDE.md` Skills table, `README.md`
and `llms.txt`.

### tf-2026-10-19-issue-rundown-and-short-commands-shipped -- issue, rundown and short commands shipped

Surfaced from bombyx's template feedback (2026-09-11). Ported three
general commands: `/issue` (work a GitHub issue end to end -- verify
the body against the tree, settle the approach, TDD, `/review` on the
pushed branch, a PR that says what was not verified; needs `gh`),
`/rundown` (grouped one-line session rundown ending with what is open)
and `/short` (restate in under 40 words). bombyx-specific steps (VM
host, its CLAUDE.md sections) were generalised.

### tf-2026-10-19-canon-prose-claims-had-no-gate -- canon prose claims had no gate

Surfaced from bombyx's template feedback (2026-09-11). Canon prose
made claims nothing checked. Ported `cargo xtask canon-check` as a
markdown-only `validate` gate after Fmt: it fails on a backticked repo
path that does not exist, a bold cross-reference introduced by "under"
that names nothing, a `git` subcommand a command file runs without
granting, prose past 80 columns, and a backlog ID cited in no backlog.
Its first run found three real defects here (an ungranted `git remote`
in `/template-backfeed`, a garbled `git -C` sentence, a dead example
ID in `/commit`). Known limit: a citation that wraps so its bold run
opens a line counts as its own target.

### tf-2026-10-19-reviewing-moved-out-of-commit-into-sequenced-rev -- reviewing moved out of commit into sequenced review

Surfaced from bombyx's template feedback (2026-09-11, four entries:
reviewing does not belong inside commit, three reviewers in sequence,
review rounds have no stopping rule, reviewers need an immutable
snapshot). `/commit` no longer reviews; the new `/review` runs
`artisan`, `red-team` and a new `fresh-reader` agent one at a time,
one lane each, against a `target/` snapshot taken with named
`git add -N` paths, with a behaviour-defect stopping rule and a
three-round cap. `code-reviewers.md`, both existing agents (snapshot
input, `AQ-`/`RT-` IDs, a Consequence field), `/implement` and
`/retrospect` were updated, and `CLAUDE.md` gained "Reviewing is its
own process". A `fresh-reader-log.md` backlog was added.

### tf-2026-10-19-auto-generated-development-diary-dropped -- auto-generated development diary dropped

Surfaced from bombyx's template feedback (2026-09-17). `/commit`
appended a narrated entry to `docs/developer/DIARY.md` on every
significant commit, producing a large write-only log that duplicated
commit bodies and `CHANGELOG.md` and mixed stale facts with current
ones. Removed the step and the file, and struck the diary from
`CLAUDE.md`, `README.md`, `llms.txt`, the architect skill and the
command files; the path stays in `NEVER_SYNC` so a sync cannot
resurrect it.

### tf-2026-10-19-issue-planning-docs-were-permanent -- issue planning docs were permanent

Surfaced from bombyx's template feedback (2026-09-17).
`docs/issues/<slug>.md` accumulated as permanent write-once records
whose reversed intermediate designs read as current. `/implement`
Phase 3 (and `/issue`) now promote durable decisions to a reference
doc or code comment and the verification note to the commit body,
then `git rm` the doc; `git rm` is granted. Existing completed issue
docs were left in place.

### tf-2026-10-19-todo-done-wrote-dangling-issue-links -- todo done wrote dangling issue links

Surfaced from bombyx's template feedback (2026-09-04). `todo done`
always linked `issues/<slug>.md`, which dangles for an item with no
doc (and now for every item, since issue docs are removed). It now
writes no link unless `--doc <path>` (relative to `docs/`) names an
existing file, guarded against blank, rooted (matched by shape, not
`Path::is_absolute`), unrenderable (allowed-set) and
repository-escaping targets; an unlinked Done entry keeps the
`- **slug** -- summary` shape the parser needs. `--slug` and `--date`
are shape-checked too (a newline could fabricate an entry), and the
caller-less `add --issue` was removed. `is_iso_date` moved to
`helpers`.

### tf-2026-10-19-code-comments-had-a-floor-but-no-rules -- code comments had a floor but no rules

Surfaced from bombyx's template feedback (2026-09-11). The coding
standards only required doc comments. `CLAUDE.md` now has a Code
comments subsection: a two-to-three-line budget (a list or a located
absence earns more) and five rules -- one reason not the chain, state
the invariant not the disaster, never inventory callers, no
hypotheticals, say what looks wrong and is not -- with reasoning
belonging in `docs/`.

### tf-2026-10-19-tagged-opening-summary-convention-added -- tagged opening summary convention added

Surfaced from bombyx's template feedback (2026-10-19). Added an
optional `CLAUDE.md` convention: open every reply with a summary under
forty words in `[short]`/`[/short]` tags, a fixed place a hook or
notifier can lift out, with no backticks or paths if it may be
spoken. `/short` is its companion; the speech hook stays per-operator.

### tf-2026-10-19-backfeed-diff-split-entries-on-indented-code-hea -- backfeed-diff split entries on indented code headers

Found while running this backfeed against bombyx (2026-10-19).
`backfeed-diff` treated an indented `### Fixed` inside an entry's code
sample as a header, silently truncating that entry's body. `hash_level`
now follows CommonMark: a header allows at most three leading spaces,
so four spaces or a tab is an indented code line, not a boundary.

### tf-2026-08-18-template-improve-only-logged-what-it-was-told -- template-improve only logged what it was told

Surfaced from bombyx's template feedback (2026-08-18).
`/template-improve` only recorded what the operator happened to
remember, losing observations already written into commits during a
session. Ported bombyx's sweep mode: with no argument the command
establishes a boundary from the feedback file's last commit, reads
four sources (commit bodies, the diary, the two review backlogs, and
thin-message diffs) plus the uncommitted working tree, applies three
judgement questions to each candidate, dedups against the file and
both backlogs, and offers the survivors for multi-select.

### tf-2026-08-18-validate-audit-could-pass-without-auditing -- validate audit could pass without auditing

Surfaced from bombyx's template feedback (2026-08-18). `validate`'s
audit step degrades to a *warning* on a missing tool or an unreachable
advisory DB, so "Validate OK" does not by itself mean the dependencies
were audited -- and a release is exactly when that answer must be
trustworthy, since an advisory is almost always filed against
already-shipped code that no changed-deps check can catch. Added a
standalone `cargo xtask audit` step to `/release` (and the release CI
job), which errors on both conditions, and noted the overstatement in
CLAUDE.md.

### tf-2026-08-18-nothing-checked-a-second-platform-before-release -- nothing checked a second platform before release

Surfaced from bombyx's template feedback (2026-08-18). Both `validate`
and `/release` ran only on the developer's platform, though the
template claims Windows, Linux and macOS as first-class -- so a green
local release could fail CI on the two platforms nobody checked. Added
a cross-platform clippy step to `/release` before the tag
(`cargo clippy --all-targets --target x86_64-unknown-linux-gnu --
-D warnings`, which must be clippy rather than `check --target`, since
`check --target` runs no lints), backed by the 3-platform CI matrix.

### tf-2026-08-18-template-shipped-no-ci-or-release-workflow -- template shipped no CI or release workflow

Surfaced from bombyx's template feedback (2026-08-18). The template
shipped no `.github` directory, so quality gates ran only where a
developer ran them and nothing checked a second platform before a
release. Added `ci.yml` (3-platform `test`, `fmt`, `clippy`, offline
`deny`, and a `frontend` job) and a minimal `release.yml` running the
release-only gates (coverage, duplication, standalone audit) on tag,
with least-privilege permissions and the cargo-deny/cargo-audit tools
installed uncached because each *is* its gate. bombyx's cross-platform
binary build/publish matrix and self-update idempotency guard were not
ported -- the template deploys rather than shipping installable
binaries.

### tf-2026-08-18-no-licence-gate-or-attribution-tooling -- no licence gate or attribution tooling

Surfaced from bombyx's template feedback (2026-08-18). The template
guarded the dependency tree for vulnerabilities and freshness but not
licences. Added `deny.toml` and a `cargo xtask deny` command -- an
offline cargo-deny licences/bans/sources gate wired into `validate`
and every-push CI, with an allow-list matched to the tree
(`unused-allowed-license = "deny"`), no `publish = false` exemption
(so `xtask` gained a `license` field), and a `helpers::
is_missing_subcommand` check so a missing cargo-deny is a hard error,
never a silent pass. bombyx's binary-archive attribution generator
(`licenses`) was deliberately not ported: the template distributes via
`cargo xtask deploy`, not published archives.

### tf-2026-08-18-commit-could-mark-breaking-before-checking-for-a -- commit could mark breaking before checking for a release

Surfaced from bombyx's template feedback (2026-08-13). `commit.md`'s
CHANGELOG step explained how to add `--breaking` / `--kind removed`
but not when not to; on a pre-1.0 scaffold, filing never-shipped
behaviour as breaking would force a spurious first release to 1.0.0.
Added a guard: confirm the behaviour actually shipped (`git tag`, or
`git log <tag>..HEAD` / the `[Unreleased]` block) before feeding the
major-bump inference, guard both `--breaking` and `--kind removed`,
and treat correcting an existing `[Unreleased]` bullet as the one
sanctioned hand edit.

### tf-2026-08-18-deferred-findings-backlog-had-no-closing-convent -- deferred-findings backlog had no closing convention

Surfaced from bombyx's template feedback (2026-08-18). `commit.md`
defined how to mint a deferred-findings backlog ID but said nothing
about retiring an entry, so annotate-vs-delete both looked correct and
a delete stranded a greppable ID cited from another doc. Added the
closing half of the convention: a fixed item leaves no entry (its
resolution is in the commit message), a standing decision moves into a
comment beside the code it governs with its revisit condition, and
`grep -rn "<id>" .` runs before any deletion.

### tf-2026-08-18-agent-editing-and-measurement-rules-worth-shippi -- agent editing and measurement rules worth shipping

Surfaced from bombyx's template feedback (2026-08-18). Two agent
habits each cost real time downstream: a slurp-mode regex
(`perl -0pi`) over YAML or code adjacent to a `///` doc comment lands
in the wrong block and fails silently, and an environment claim
written from expectation rather than measurement is a plausible wrong
answer nothing catches. Added both rules to CLAUDE.md Collaboration --
use `Edit` not a slurp regex near indentation/doc-comments, and print
the variable before claiming what it holds.

### tf-2026-08-18-powershell-expands-dollar-vars-in-xtask-prose-ar -- PowerShell expands dollar vars in xtask prose args

Surfaced from bombyx's template feedback (2026-08-13). On Windows a
double-quoted PowerShell string passed to `changelog add`,
`feedback-add --title` or `todo add` is expanded before the program
sees it: a known variable like `$HOME` leaks a real home path into a
tracked file, and an unknown one like `$XDG_CONFIG_HOME` silently
becomes the empty string. Added an Environment Constraints note
prescribing a single-quoted PowerShell string (or a Bash heredoc) for
`$`-bearing xtask prose, covering both failure modes.

### tf-2026-08-18-cli-only-prune-stranded-skills-json-and-command -- CLI-only prune stranded skills.json and command prose

Surfaced from bombyx's template feedback (2026-08-18 and 2026-08-10).
Pruning to a CLI-only project removed crates and xtask code but left
`.claude/skills.json` registering a `web-dev` skill (dangling path
plus a false Axum/Svelte/Vite description) and frontend assumptions
woven through `.claude/commands/*.md` prose -- neither of which the
compiler can find. Added a `skills.json` edit step and a prose-sweep
step (grep `.claude/`, `CLAUDE.md`, `llms.txt`, `README.md`, `docs/`
for the removed names) to the CLI-prune checklist.

### tf-2026-08-18-clean-cache-did-not-compile-off-windows -- clean_cache did not compile off Windows

Surfaced from bombyx's template feedback (2026-08-18).
`clean_cache.rs` imported `is_reparse_or_symlink_meta` unconditionally
but used it only in the `#[cfg(windows)]` branch, so under the
workspace `warnings = "deny"` it was an unused-import *compile error*
on Linux and macOS; and off Windows the wrapper never returns `Err`,
tripping `clippy::unnecessary_wraps`. Gated the import to its use site
and added a `cfg_attr(not(windows))` allow, verified with a
cross-target `cargo clippy --target x86_64-unknown-linux-gnu` run.

### tf-2026-08-18-validate-step-numbers-were-hand-maintained-liter -- validate step numbers were hand-maintained literals

Surfaced from bombyx's template feedback (2026-08-18). `validate.rs`
held a `TOTAL_STEPS` const and passed each step its own literal index,
so adding a gate meant renumbering every later call and a miss printed
`[5/9]` twice with no test failing. Replaced with a `Step` table whose
per-step index and total both derive from one list, and added a test
resolving every step's iterate-with `cmd` through the real CLI so a
renamed subcommand cannot leave the hint pointing at a command that no
longer exists.

### tf-2026-08-18-crate-manifest-readme-pointed-outside-the-packag -- crate manifest readme pointed outside the package

Surfaced from bombyx's template feedback (2026-08-10). The primary
crate's `readme = "../../README.md"` points outside the package root,
so `cargo package` ships a manifest referencing a file not in the
crate; clippy's `cargo` lint group also requires `package.readme`, so
the key cannot simply be dropped -- the trap only surfaces at publish
time, which the local validate gate never reaches. Shipped a short
`crates/rustbase/README.md` and repointed the key to it.

### tf-2026-08-18-xtask-check-compiled-only-lib-and-bin-targets -- xtask check compiled only lib and bin targets

Surfaced from bombyx's template feedback (2026-08-13). `cargo xtask
check` ran `cargo check --workspace` with no `--all-targets`, so it
compiled only the library and binary and skipped tests/benches/
examples -- a broken test call site (an altered signature) still
printed `Check OK` and only failed later under `cargo xtask test`.
Added `--all-targets` in a named `CHECK_ARGS` const with a unit test
pinning the flag, and corrected the now-misleading "fast / no tests"
wording across CLAUDE.md, llms.txt, README.md, the clap help, and the
`/check` command doc.

### tf-2026-07-24-implement-pre-launch-signal-for-undo-inverse-opt -- Implement pre-launch signal for undo/inverse/optimistic-concurrency

Surfaced from jutro's template feedback (2026-07-23). The
`/implement` Phase 3 pre-launch step lists signals that a diff
is review-volatile (so pre-launching reviewers would waste the
work), but it did not name the design shapes reviewers most
often reshape. Added "introduces an undo / inverse /
optimistic-concurrency path" to that signal list: those designs
hinge on a backend contract an adversarial review routinely
overturns -- an undo that looks right for the simple case but
breaks the recurring / concurrent one -- so the diff gets
reshaped after review and the heavy gates would otherwise run
twice. Edit is in `.claude/commands/implement.md` step 5.

### tf-2026-07-24-retrospect-findings-restructured-into-short-long -- Retrospect findings restructured into Short/Long/Fix

Surfaced from jutro's template feedback (2026-07-23). The
`/retrospect` report emitted a free-form description blob per
finding, which was hard to skim and let awareness-only items
("be aware of X") slip in without a concrete action. Restructured
each finding into three explicit fields -- a one-line **Short**
(skimmable claim), a **Long** (detail with turn/tool refs), and
a concrete **Fix** (the specific edit/command/deletion) -- and
made a concrete fix mandatory: a finding with no actionable fix
is dropped during surfacing and never shown. Updated the Tagging,
Presenting (example output), and Rules sections of
`retrospect.md`; the template's existing four buckets were kept
(jutro's extra "Architectural insights" bucket was not part of
this entry).

### tf-2026-07-24-mechanical-changelog-todo-xtask-commands -- Mechanical changelog + todo xtask commands

Surfaced from jutro's template feedback (2026-07-23).
`CHANGELOG.md` and `docs/todo.md` were hand-edited, hitting the
documented "grep the `### ` headings first or you split a block"
CHANGELOG footgun and forcing a full read of a large `todo.md`
just to append one bullet or check a slug. Added
`cargo xtask changelog add --kind <k> [--breaking] "text"`
(finds the right `[Unreleased]` subsection, creating it in
canonical order only if absent) and `cargo xtask todo
<list|add|done>` (list prints `slug -- summary`; add appends
with a slug-uniqueness check; done moves a bullet to the top of
`## Done`, dated and issue-linked), both built on new shared
`helpers.rs` markdown primitives (`wrap_markdown`, `rejoin`,
`section_bounds`, `to_owned_lines`, `MARKDOWN_WIDTH`). Wired
`/commit` (CHANGELOG step), `/todo`, and `/implement` (Done
move) to call them. jutro's `changelog infer-bump`/`promote`
subcommands were left out -- they overlap the template's
`/release` design and were outside this feedback's scope.

### tf-2026-07-24-promote-code-reviewers-to-first-class-read-only -- Promote code reviewers to first-class read-only agents

Surfaced from jutro's template feedback (2026-07-23). The Red
Team and Artisan reviewer prompts shipped as inline quote blocks
inside `code-reviewers.md`, spawned as generic agents, so
"do not modify files" was only an instruction and the persona
lived tangled with the orchestration. Promoted each to a
first-class subagent under `.claude/agents/` (`red-team.md`,
`artisan.md`) with read-only tool sets -- `red-team` keeps
`Bash` for its `git log` history pass; `artisan` has no shell
and is read-only by construction. `code-reviewers.md` is now a
small gating-rules doc, and `/commit` (step 3) and `/implement`
(Phase 3 pre-launch) spawn the reviewers by `subagent_type`.
jutro's project-specific `dogfooder`/`mobile-ux` agents were
deliberately not backfed.

### tf-2026-07-18-harden-template-sync-judge-tooling-by-consumer-m -- Harden /template-sync: judge tooling by consumer, measure divergence, adapt absent-machinery refs

Surfaced from clockdump's template feedback (2026-07-18).
During a v0.14 -> v0.17 sync, three gaps in
`.claude/commands/template-sync.md` made the run harder than
it should have been. First, incoming `xtask` tooling and
command reworks were nearly skipped as "template-internal"
even though they back commands derived projects run
(`sync-candidates`/`feedback-add`); step 5 now judges tooling
by its consumer and treats a command rework and its backing
subcommand as coupled. Second, local divergence was
eyeballed rather than measured; step 7 now runs
`git diff <last-synced>:<file> HEAD:<file>` to classify
clean-adopt vs genuine merge. Third, adaptation was limited
to the crate-name rename; step 7 now greps applied files for
project-absent machinery (e.g. `cargo xtask deploy` in a CLI
that removed its deploy flow) and adapts or drops those
references. clockdump applied all three locally before
suggesting them upstream.

### tf-2026-07-16-dep-age-check-moved-to-first-validate-step -- dep-age-check moved to first validate step

Surfaced from ledgerstone's template feedback (2026-07-16).
`dep-age-check` ran as the last `validate` step; moved it to
step 1 so a dependency adopted within the cooldown fails the gate
before the compile steps (Clippy, Test, Coverage) build and run
its build script on the too-new crate. It is a no-op on unchanged
lockfiles, so the early placement costs nothing on the common
commit, and a connectivity failure still degrades to a warning
rather than blocking the local gates. Audit stays last.

### tf-2026-07-16-backfeed-ledger-name-override-for-worktree-layou -- Backfeed ledger --name override for worktree layouts

Surfaced while backfeeding ledgerstone (2026-07-16), whose
checkout lives at `../ledgerstone/main`. The backfeed ledger keyed
the downstream by the final path component, yielding the branch
name `main` -- a poor, collision-prone key (any worktree-style
`<project>/<branch>` layout would clash). Added a `--name`
override to `backfeed-diff` / `backfeed-record` (falling back to
the basename) so worktree-layout downstreams are keyed by their
real project name.

### tf-2026-07-16-systemd-startlimit-keys-moved-to-unit-section -- systemd StartLimit keys moved to Unit section

Surfaced from ledgerstone's template feedback (2026-07-16). The
shipped `deploy/rustbase-web.service` placed `StartLimitIntervalSec`
and `StartLimitBurst` in `[Service]`, where modern systemd
silently ignores them -- so the crash-loop restart cap never
applied and a persistently-failing service would restart forever.
Moved both keys to `[Unit]` and switched `After=network.target` to
`Wants=network-online.target` + `After=network-online.target` so an
externally-bound server has a usable address before start.

### tf-2026-07-16-single-const-for-the-primary-crate-manifest-path -- Single const for the primary crate manifest path

Surfaced from ledgerstone's template feedback (2026-07-16).
`deploy_guard.rs` hard-coded `crates/rustbase/Cargo.toml` in the
release-version check, so a renamed downstream re-patched it on
every sync. Collapsed the functional occurrence to a
`PRIMARY_MANIFEST` const (one edit point). Full workspace-member
resolution was judged not worthwhile: the `[workspace] members`
array has no "primary crate" marker, and it cannot fix the
`/release` command doc, which references the path in prose.

### tf-2026-07-16-dep-age-percent-encodes-the-package-name -- dep-age percent-encodes the package name

Surfaced from ledgerstone's template feedback (2026-07-16).
`dep_age::fetch_registry` interpolated the raw package name into
the registry URL, so a scoped npm name (`@scope/name`) 404'd
because the `/` was not URL-encoded -- and the `dep-age-check` /
`dep-preflight` gates would misreport a scoped transitive
dependency as unavailable. Added `percent_encode_segment`
(RFC-3986 unreserved set, `@` kept literal, `/` -> `%2F`) applied
to both the npm and crates.io URLs, with a unit test.

### tf-2026-07-16-backfeed-only-helpers-decoupled-for-partial-adop -- Backfeed-only helpers decoupled for partial adoption

Surfaced from ledgerstone's template feedback (2026-07-16). The
v0.16.0 template-maintenance helpers all lived in `helpers.rs`,
but `extract_iso_date` / `is_iso_date` are consumed only by
`backfeed.rs`. A downstream adopting just the sync/feedback half
and deleting the template-repo-only `backfeed` module hit dead
code under `warnings = deny`. Moved both into `backfeed.rs` so the
sync/feedback half is adoptable without carrying `backfeed.rs`.

### tf-2026-07-16-no-release-semver-workflow-commit-bundled-versio -- no `/release` SemVer workflow; `/commit` bundled version bumping

Surfaced from jutro's template feedback (2026-07-14 /
2026-05-21). `/commit` bumped the version, synced
`Cargo.lock`, and ran `validate` on every feat/fix/perf --
treating every functional commit as a release, which does
not survive contact with real projects (many commits land
between deploys). **Fix:** split the workflow. Added
`.claude/commands/release.md` -- the sole version-bumper: it
infers the bump from the accumulated `[Unreleased]` CHANGELOG
entries (`**BREAKING:**`/`### Removed` -> major, `### Added`
-> minor, else patch), promotes `[Unreleased]` to a dated
section, runs `validate` as the release gate, commits the
bookkeeping directly, and creates an annotated tag.
Restructured `commit.md` into a save-point (no bump, no
`Cargo.lock` sync, no auto-validate; diary entries drop the
`(vX.Y.Z)` suffix; CHANGELOG bullets mark breaking changes).
Added a `deploy_guard` (`xtask/src/deploy_guard.rs`) that
refuses `cargo xtask deploy` unless `HEAD` is on a `vX.Y.Z`
annotated tag matching `Cargo.toml` with a clean tree -- with
a `[section]`-tracking TOML version parser so a dependency's
`version` cannot shadow `[package]`. Generalized jutro's
`cargo generate-lockfile` to `cargo update -p rustbase`
(template canon forbids the former; it would trip
`dep-age-check`). Updated CLAUDE.md "Commits and releases",
"Semantic Versioning", "Release Notes", and the Skills table.

### tf-2026-07-16-no-local-html-report-command-only-the-cloud-arti -- no local HTML-report command (only the cloud Artifact path)

Surfaced from jutro's template feedback (2026-07-14). The
template's only visual-document path was the cloud `Artifact`
tool; operators who need a page to stay on the machine had no
built-in option. **Fix:** shipped `.claude/commands/
html-report.md` plus its single-source-of-truth design asset
`docs/ai-agents/html-report-template.html`. The command copies
the in-repo template, swaps palette + content, and opens a
self-contained local file (all CSS/JS inlined, images as
`data:` URIs, CSP-`<meta>`-enforced; fetched content treated
as data and HTML-escaped; a `[a-z0-9-]+` slug guard; one
`--fs-*` type scale; dual theming). Generalized jutro's two
project-specific examples ("journal contents", "malina host")
to generic "personal data / deploy hostnames and paths".

### tf-2026-07-16-stub-integration-tests-were-unmarked-replace-me -- stub integration tests were unmarked replace-me scaffolding

Surfaced from jutro's template feedback (2026-06-07). The
`Hello from`/`verbose mode` assertions in
`crates/rustbase/tests/integration_test.rs` are pinned to the
stub binary's example output and must be rewritten the moment
a real subcommand lands, but nothing said so. **Fix:** added a
header comment marking those two tests as replace-me
scaffolding and pointing to `cli_version_flag` /
`cli_help_flag` (which assert only version/help and survive
the first real command) as the model for new tests.

### tf-2026-07-15-dependency-cooldown-was-post-resolution-only-no -- dependency cooldown was post-resolution only (no build-host protection)

Surfaced from clockdump's template feedback (2026-07-15).
`dep-age-check` runs *after* cargo has resolved, fetched, and
compiled a change, so a freshly-published crate's build script
has already run on the build host by the time the gate fails --
it protects the committed lockfile, not the machine. clockdump
suggested migrating to cargo's in-resolver `-Zmin-publish-age`
once its client side stabilizes on stable. **Fix:** added
`cargo xtask dep-preflight`, a front-door pre-compile
remediation for the Rust tree -- it reads the changed crates
(same `HEAD` diff as the gate) and pins each one still within
the cooldown down to its newest aged version via `cargo update
--precise`, looping until the set is aged or no aged version
fits the resolved requirements. Every step touches only the
registry index and the lockfile, so no build script runs until
the tree is clean. Documented the post-resolution limitation
and the `-Zmin-publish-age` migration trigger in CLAUDE.md's
Supply-chain hygiene section. The pure loop logic is
unit-tested via injected I/O; the shell-outs are thin.

### tf-2026-05-18-clean-cache-windows-junctions-bypassed-the-symli -- `clean_cache` Windows junctions bypassed the symlink guard

Surfaced from hoard's template feedback (2026-05-18).
`delete_entry` in `xtask/src/clean_cache.rs` dispatched
on `FileType::is_symlink()`, but on Windows that flag
is only set for `IO_REPARSE_TAG_SYMLINK` -- directory
junctions (`mklink /J`) returned `false` and fell into
the `is_dir()` branch, where `remove_dir_all` would
traverse the junction and delete the *target* tree
outside the workspace. **Fix:** added an
`is_reparse_or_symlink` helper that checks
`FILE_ATTRIBUTE_REPARSE_POINT` via
`MetadataExt::file_attributes` on Windows; reparse
points unlink via `remove_dir` with a `remove_file`
fallback. Added a `cmd /c mklink /J`-based regression
test (no elevation required, unlike `symlink_dir`).

### tf-2026-05-18-profile-release-overrides-shipped-slow-deployed -- `[profile.release]` overrides shipped slow deployed binaries

Surfaced from hoard's template feedback (2026-05-18).
Root `Cargo.toml` set `[profile.release] incremental =
true, codegen-units = 256`. The accompanying doc
correctly named "any project shipping a binary to
multiple users" as a case to skip -- but every derived
project's `cargo build --release` (including deploy
flows) silently inherited a measurably slower binary
for no iteration-speed gain. **Fix:** moved the
overrides into a new `[profile.release-fast]` that
inherits from `release`; `[profile.release]` stays at
cargo defaults so deployed binaries are fully
optimised. Local fast iteration uses
`cargo build --profile release-fast`.

### tf-2026-05-18-stop-check-sh-chain-dropped-earlier-stage-output -- `stop-check.sh` `&&` chain dropped earlier-stage output

Surfaced from hoard's template feedback (2026-05-18).
The hook ran `output=$(cargo fmt --check && cargo
xtask clippy && cargo xtask test)`. On multi-stage
failure the captured `$output` held only the
short-circuited stage's output with no label, so when
Claude fixed clippy the previously-masked test
failure resurfaced looking "new" -- exactly the
fix-loop amplification the `stop_hook_active` guard
is meant to prevent. **Fix:** extracted a `run_stage
"label" cmd...` helper that runs stages sequentially
and prefixes failing output with `=== <stage> failed
===`, so Claude sees both the stage label and the
full failure body on the first hook run.

### tf-2026-05-18-dir-size-was-a-library-helper-that-printed-to-st -- `dir_size` was a library helper that printed to stderr and undercounted silently

Surfaced from hoard's template feedback (2026-05-18).
`dir_size` in `xtask/src/helpers.rs` returned
`Ok(total)` but called `eprintln!("warning: ...
(skipping)")` from inside the recursive walk on
per-entry failures, contradicting its
`Result<u64, String>` contract ("size or error") with
"best-effort size plus stderr side-effects." **Fix:**
changed the signature to
`Result<(u64, Vec<String>), String>` so per-entry
failures populate a warnings vector and callers
decide what to do. `clean_cache::clear_dir_contents`
folds those warnings into its own per-entry error
list, so warnings now surface alongside deletion
failures in the final report instead of being dumped
to stderr mid-loop.

### tf-2026-05-18-temp-scratch-used-fragile-threadid-debug-parsing -- `temp_scratch` used fragile `ThreadId` debug parsing and bare `unwrap`

Surfaced from hoard's template feedback (2026-05-18).
`temp_scratch` in `xtask/src/helpers.rs` formatted
`thread::current().id()` via `{:?}` and filter-to-
digits to build a unique suffix. `ThreadId`'s `Debug`
output is unspecified by std and could change across
Rust versions, and the process-wide `AtomicU64`
counter is already unique across parallel test
threads, so the thread-id component was redundant.
The trailing `fs::create_dir_all(&dir).unwrap()` also
gave a generic panic that misled CI diagnostics.
**Fix:** dropped the `ThreadId` parsing (kept
`pid + AtomicU64`) and replaced `unwrap` with
`unwrap_or_else` carrying a descriptive panic that
names the failing path.

### tf-2026-05-18-template-sync-didn-t-document-the-windows-git-sh -- `/template-sync` didn't document the Windows `git show <rev>:<path>` failure

Surfaced from hoard's template feedback (2026-05-18).
On Windows shells the `:` separator in `git show
template/main:crates/rustbase/Cargo.toml` (and
similar `revspec:path` forms) can be mangled into a
`;` separator, producing an "ambiguous argument"
error with no obvious workaround. **Fix:** added a
"Windows note" to step 9 of
`.claude/commands/template-sync.md` prescribing the
cross-platform `git show <rev> -- <path>` /
`git diff <rev> -- <path>` form, which keeps the
path as a separate argument and sidesteps the colon
mangling. The note also covers any step-7 site that
reads upstream files via `revspec:path`.

### tf-2026-05-17-xtask-check-aborting-filter-dropped-legitimate-u -- `xtask check` "aborting" filter dropped legitimate user errors

Surfaced from rustwerk's template feedback (2026-04-19,
fixed there locally). `extract_error_lines` in
`xtask/src/check.rs` used `!l.contains("aborting")` to
strip rustc's `error: aborting due to N previous errors`
summary line. Any legitimate user error whose message
contained the substring "aborting" (e.g.
`compile_error!("aborting: feature X required")`) was
also silently filtered out, producing `FAILED: 0
compilation error(s)` with no body. **Fix:** anchored
the filter to the exact summary prefix
(`!l.starts_with("error: aborting due to")`) and added
a regression test (`keeps_user_errors_mentioning_aborting`).

### tf-2026-05-17-xtask-check-reported-0-compilation-error-s-for-n -- `xtask check` reported "0 compilation error(s)" for non-rustc failures

Surfaced from rustwerk's template feedback (2026-04-19).
When cargo exited non-zero for reasons other than
compile errors (manifest parse failure, corrupted
`Cargo.lock`, missing registry network, unsupported
flag on older cargo), the first diagnostic line did
not match the `error[`/`error:` prefix filter, so the
output became `FAILED: 0 compilation error(s)` with
nothing to diagnose. **Fix:** when `errors.is_empty()`
but the process exited non-zero, `check()` now prints
the last ~20 non-empty stderr lines verbatim under a
`FAILED: cargo exited non-zero with no matched error
lines` banner so the user sees an actionable signal.

### tf-2026-05-17-clippy-pedantic-doc-markdown-flagged-common-infr -- `clippy::pedantic` `doc_markdown` flagged common infra identifiers

Surfaced from rustwerk's template feedback (2026-04-21).
Writing doc comments that mention `PowerShell`, `FFI`,
`JSON`, `WebSocket`, `macOS`, `GitHub` and similar
well-known terms required each occurrence to be
backticked under `clippy::doc_markdown`, producing
prose with unnecessary backticks for the reader.
**Fix:** added a workspace-root `clippy.toml` with a
curated `doc-valid-idents` allowlist that extends
clippy's defaults via the `".."` sentinel. Documented
in CLAUDE.md under "Lints: `doc_markdown` allowlist
via `clippy.toml`" with a note that derived projects
should append, not redefine.

### tf-2026-05-17-template-sync-granted-bash-git-checkout-it-never -- `/template-sync` granted `Bash(git checkout:*)` it never used

Surfaced from rustwerk's template feedback (2026-04-19).
The slash command's `allowed-tools` front-matter
included `Bash(git checkout:*)`, but the documented
workflow only used `git diff`, `show`, `log`,
`rev-parse`, `fetch`, and `status` for reading, plus
`Edit` / `Write` for applying. The glob permitted
destructive variants (`checkout -f <ref>`,
`checkout -- .`, `checkout -- <path>`) that can
destroy uncommitted work or move HEAD onto an
untrusted template ref whose `.gitattributes` /
hooks could activate on the next git command --
combined with diff content being LLM input, that's
an escape vector. **Fix:** removed
`Bash(git checkout:*)` from the allowed-tools list
in `.claude/commands/template-sync.md`.

### tf-2026-05-17-template-sync-trusted-upstream-url-from-template -- `/template-sync` trusted upstream URL from `.template-sync.toml`

Surfaced from rustwerk's template feedback (2026-04-19).
Step 3 of the workflow read `repo` from
`.template-sync.toml` and fed it to
`git remote add template <url>`. A malicious PR
changing that field (or a typo-squat lookalike)
would silently redirect future syncs; `git remote
add` also historically accepted hostile URL forms
(`ext::sh`, `--upload-pack=...`). **Fix:**
hard-coded the expected upstream
(`https://github.com/breki/rustbase`) in the slash
command. The command now asserts the
`.template-sync.toml` value matches exactly before
proceeding, refuses anything outside the
`https://github.com/breki/` prefix, and verifies any
pre-existing `template` remote URL also matches.
Bootstrap flow updated to use the same hard-coded
URL.

### tf-2026-05-17-template-sync-all-option-bypassed-per-file-revie -- `/template-sync` "all" option bypassed per-file review

Surfaced from rustwerk's template feedback (2026-04-19).
Step 6 accepted "all" to apply every recommended
change in bulk. Because the agent reads raw upstream
commit messages and diff bodies during step 5
categorization, a single compromised upstream commit
could smuggle an instruction that bulk-apply would
honour. **Fix:** removed "all" as an accepted answer.
Users must opt in by category name or specific file
path; "none" still works as a no-op marker update.

### tf-2026-05-17-no-pwsh-twin-convention-for-sh-scripts -- No pwsh-twin convention for `.sh` scripts

Surfaced from rustwerk's template feedback (2026-04-21).
The template targeted Windows as a first-class
platform but had no convention for how `scripts/*.sh`
should reach Windows users -- ad-hoc `.ps1` ports
risked duplicating bash logic that then drifted on
bugfixes. **Fix:** added a "Shell wrappers" section
to CLAUDE.md prescribing "non-trivial logic lives in
`cargo xtask`; shell files are thin wrappers only,"
with canonical bash and pwsh wrapper shapes and an
explicit carve-out for cases that can't live in Rust
(process cleanup, pre-cargo bootstrap).

### tf-2026-05-17-cli-only-downstream-had-no-documented-safe-to-de -- CLI-only downstream had no documented "safe to delete" list

Surfaced from rustwerk's template feedback (2026-04-19,
`N/A for rustwerk` note). The template's full-stack
shape meant CLI-only downstreams had to discover by
trial which directories were safe to remove. **Fix:**
CLAUDE.md's "Workspace Crates" section now lists the
full set safe to delete for a CLI-only project
(`crates/rustbase-web/`, `frontend/`, `e2e/`,
`.mise.toml`, `playwright.config.*`, `.ports*`,
`scripts/e2e.sh`) along with the corresponding
`Cargo.toml`, `xtask validate`, and `build.ps1`
pruning steps, and notes that `/template-sync` will
auto-skip those paths once they no longer exist.

### tf-2026-05-17-gitignore-only-hid-root-anchored-target -- `.gitignore` only hid root-anchored `/target/`

Surfaced from trmnl-bellwether's template feedback
(2026-04-19). The template's `.gitignore` had
`/target/`, so any nested `target/` (e.g.
`crates/<name>/target/` created when cargo was invoked
from inside a crate directory) showed up as untracked
and was easy to accidentally `git add .` into a commit.
**Fix:** replaced `/target/` with `target/` plus
`**/target/` for belt-and-braces, so any `target/`
directory at any depth is ignored.

### tf-2026-05-17-cargo-xtask-test-had-no-ignored-forwarding -- `cargo xtask test` had no `--ignored` forwarding

Surfaced from trmnl-bellwether's template feedback
(2026-04-19). CLAUDE.md forbids raw `cargo test`, but
the `xtask test` wrapper had no way to run
`#[ignore]`-tagged tests (the standard Rust idiom for
"manual tool" tests that shouldn't run in
`validate`), so projects using `#[ignore]` for manual
tools hit a dead end. **Fix:** added a `--ignored`
flag to `XCommand::Test`, introduced
`TestOptions<'a> { filter, verbose, ignored }` so the
signature stays readable as flags accrete, threaded
`--ignored` through `build_args` after an explicit
`--` separator, and added two `build_args` unit tests
covering ignored-alone and ignored-plus-filter.
CLAUDE.md's Build Commands table updated to advertise
`cargo xtask test --ignored`.

### tf-2026-05-17-commit-skill-ambiguous-about-how-to-hand-the-dif -- `/commit` skill ambiguous about how to hand the diff to review subagents

Surfaced from trmnl-bellwether's template feedback
(2026-04-17). The previous wording ("Pass the full
`git diff` output to both agents and tell them to
read the relevant source files") didn't prescribe a
mechanism, so models reflexively reached for
`tokio::fs::write("/tmp/foo-diff.txt", ...)` or
`git diff --cached > /tmp/foo-diff.txt`. On Windows
with Git Bash, `/tmp` maps outside the workspace,
isn't git-ignored, and is invisible to the user.
**Fix:** the skill now explicitly tells each subagent
to run `git diff --cached` itself as its first step
(both agents have Bash), forbids `/tmp` paths, and if
a file is genuinely needed prescribes a
git-ignored workspace-local path under `target/`. The
mechanism is also reusable: any future
"capture-output-and-hand-to-subagent" pattern in the
skill should prefer subagent-runs-the-command or
`target/`-local files.

### tf-2026-05-17-commit-resolved-log-entry-format-was-unspecified -- `/commit` resolved-log entry format was unspecified

Surfaced from trmnl-bellwether's template feedback
(2026-04-17). The skill said "remove from the open
log and insert at the top of the resolved log with
the fix date and resolution" but was ambiguous about
whether to preserve the original Description / Impact
/ Suggested-fix body or replace it with a terse
resolution-only entry. Different agents picked
different formats across sessions, causing cross-PR
inconsistency in the same project's resolved log.
**Fix:** the skill now prescribes a terse format
(`### <ID> -- <title>` heading + `**Category:**` line
+ `**Resolution:**` line with the fix date and a 1-3
sentence note) and explicitly says not to preserve
the original body in the resolved entry.

### tf-2026-05-17-commit-changelog-rule-keyed-on-commit-type-not-o -- `/commit` CHANGELOG rule keyed on commit type, not observable effect

Surfaced from trmnl-bellwether's template feedback
(2026-04-17). The skill said "Skip [CHANGELOG] for:
chore, ci, style, docs-only changes." But chores
routinely contain user-visible behaviour changes
(e.g., a default port change committed as `chore:`)
where a CHANGELOG entry is genuinely needed. **Fix:**
the rule now keys on user-observable effect, not
commit type: add a `[Unreleased]` entry whenever a
user of the software would see a difference (new
feature, fixed bug, changed default, removed flag,
new config knob, port change, new env var, ...) --
even if the commit type is `chore`. Skip only for
commits with no user-observable effect (pure
refactors, internal tooling, test-only changes,
docs-only edits, CI/lint tweaks invisible to users).

### tf-2026-05-17-stop-hook-ran-full-cargo-xtask-validate-includin -- Stop hook ran full `cargo xtask validate`, including ~15s coverage step

Surfaced from kozmotic's template feedback (2026-05-04).
The Stop hook ran `cargo xtask validate` -- fmt +
clippy + tests + coverage + duplication + frontend
check -- on every stop where Rust files had changed.
Coverage alone adds ~15s on a small codebase, and the
Stop hook fires often enough during interactive work
that the cost compounded. **Fix:** Stop hook now runs
`cargo xtask clippy && cargo xtask test` only,
skipping coverage, duplication, fmt-check, and the
frontend type check. Full validate still runs from
`/commit` (and is available manually) so coverage and
duplication signal is preserved at the commit gate;
the Stop hook is positioned purely as a fast
interactive safety net for lint + test regressions.

### tf-2026-05-17-scripts-shipped-5-trivial-one-line-cargo-wrapper -- `scripts/` shipped 5 trivial one-line cargo wrappers redundant with xtask

Surfaced from kozmotic's template feedback (2026-05-04).
The template shipped `scripts/build.sh`, `clippy.sh`,
`test.sh`, `fmt.sh`, `validate.sh` -- each a one-line
`cargo ...` wrapper. CLAUDE.md already steers users to
`cargo xtask` ("Never use raw `cargo test`"), so the
shell wrappers added discoverability noise without
adding capability. **Fix:** deleted the 5 trivial
wrappers. Kept `scripts/e2e.sh` and
`scripts/kill-servers.sh` -- both contain non-trivial
process-cleanup logic (PowerShell `Get-CimInstance`
filtering, pkill patterns) that doesn't fit naturally
as an xtask subcommand.

### tf-2026-05-17-no-documented-pattern-for-hardware-bound-code-un -- No documented pattern for hardware-bound code under the 90% coverage gate

Surfaced from kozmotic's template feedback (2026-05-04).
The 90% coverage gate assumes everything is testable,
but real CLI projects have I/O paths (audio, network,
native APIs) that can't run in CI. Kozmotic's working
recipe -- extract the hardware-bound call into a
sibling submodule, add the submodule to the coverage
`IGNORE_REGEX`, and add a `*_TEST_*` env-var escape
hatch so the parent module's success/error branches
remain in the gate -- was undocumented. **Fix:** added
a "Coverage exceptions for hardware-bound code" section
to `CLAUDE.md` next to the existing workspace-lints
override recipe, describing the three-step pattern and
the explicit "use a fake trait if you can" caveat.

### tf-2026-04-16-eslint-js-pin-in-frontend-package-json-was-unres -- `@eslint/js` pin in `frontend/package.json` was unresolvable

The template shipped `"@eslint/js": "^10.2.0"`, but
10.0.1 is the latest published version on npm, so
`npm install` from a clean clone failed with
`ETARGET`. The lockfile had 10.0.1 installed, masking
the problem until someone ran `install` without the
lock. **Fix:** changed the pin to `^10.0.0`. Template
should pin to a version that actually exists and
ideally be kept in sync by a bot like Renovate.

### tf-2026-04-16-deploy-logic-belongs-in-xtask-not-bash-scripts -- Deploy logic belongs in xtask, not bash scripts

Initially deferred for a dedicated design discussion
because hoard's deploy was specific to a Raspberry Pi
/ SSH / systemd workflow. Resolved in v0.5.0 by
porting hoard's deploy-as-xtask pattern (generalized
for any Linux+systemd target) into the template.

### tf-2026-05-17-vite-config-js-uses-commonjs-dirname-in-esm-cont -- `vite.config.js` uses CommonJS `__dirname` in ESM context

`frontend/package.json` declares `"type": "module"`,
which means all `.js` files are ES modules. However,
`vite.config.js` used `__dirname` (lines 9, 22), which
is a CommonJS-only global. This worked only because
Vite's config loader shims `__dirname`, but it is not
idiomatic and would break if copy-pasted elsewhere.

**Fix:** Replaced with `import.meta.dirname`
(available since Node 21.2 / 22+).

### tf-2026-05-17-tokio-used-features-full-unnecessarily -- Tokio used `features = ["full"]` unnecessarily

`crates/rustbase-web/Cargo.toml` depended on
`tokio = { version = "1", features = ["full"] }`. The
`full` feature flag pulled in every Tokio subsystem
including unused ones, increasing compile time and
binary size.

**Fix:** Replaced with explicit feature list
(`["macros", "rt-multi-thread", "net", "signal"]`).

### tf-2026-05-17-health-endpoint-returned-plain-text-not-json -- `/health` endpoint returned plain text, not JSON

The `/health` endpoint returned `"OK"` as plain text,
while `/api/status` and `/api/greeting` both returned
JSON. Inconsistent for API consumers and orchestrators
that expect JSON.

**Fix:** `/health` now returns `{ "status": "ok" }`
with `Content-Type: application/json`.

### tf-2026-05-17-release-workflow-assumed-7z-is-available-on-wind -- Release workflow assumed `7z` is available on Windows

`.github/workflows/release.yml` used `7z a "${STAGING}.zip" ...`.
GitHub-hosted Windows runners include 7-Zip, but
self-hosted runners may not.

**Fix:** Replaced with PowerShell's built-in
`Compress-Archive`, requiring no external tools.

### tf-2026-05-17-release-notes-extraction-from-changelog-was-frag -- Release notes extraction from CHANGELOG was fragile

The release workflow used an `awk` script to extract
notes by heading match. Format deviations would
silently produce an empty `release_notes.md`.

**Fix:** Release workflow now warns (and later fails)
when CHANGELOG extraction produces empty release notes;
extraction uses exact version match instead of
substring.

### tf-2026-05-17-no-frontend-linting-or-formatting-tools -- No frontend linting or formatting tools

The Rust side had strict quality gates; the frontend
had nothing comparable. No ESLint config, no Prettier
config; `cargo xtask validate` did not check frontend
code quality.

**Fix:** Added ESLint with `eslint-plugin-svelte` and
Prettier with `prettier-plugin-svelte`. `lint`,
`format`, `format:check` scripts in
`frontend/package.json`. `svelte-check` step added to
`cargo xtask validate`.

### tf-2026-05-17-no-frontend-unit-test-infrastructure -- No frontend unit test infrastructure

The template included Playwright for E2E but had no
setup for unit/component tests in isolation.

**Fix:** Added Vitest with `@testing-library/svelte`
and `jsdom`; `test` script wired into the frontend.

---

## Suggestions to flow back to the template

(In a derived project, this section lists ideas the
project wants to push upstream into the template. In
this template repo itself there is no upstream, so the
section is normally empty. It is preserved so new
entries route identically across template and derived
projects -- and so the file's section shape ships as
part of the template.)
