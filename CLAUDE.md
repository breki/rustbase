# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code)
when working with code in this repository.

**IMPORTANT: The working directory is already set to the
project root. NEVER use `cd` to the project root or
`git -C <dir>` -- blanket permission rules cannot be
set for commands starting with `cd` or `git -C`, so
they require manual approval every time.**

## Project Overview

<!-- TODO: describe your project here -->

- **Stack**: Rust/Axum backend, Svelte 5/Vite frontend
- **Target platforms**: Windows, Linux, macOS

### Workspace Crates

| Crate | Purpose |
|-------|---------|
| `crates/rustbase` | Core library and CLI binary |
| `crates/rustbase-web` | Axum web server (optional) |
| `crates/rustbase-xtask` | Shared xtask gates, as a library published on crates.io (`docs/xtask-library.md`) |
| `crates/rustbase-updater` | Self-upgrade engine for apps updating from signed release tags, a library published on crates.io (`crates/rustbase-updater/README.md`) |
| `xtask` | Build automation (calls `rustbase-xtask`) |

The web crate is optional. For a CLI-only project,
follow these steps in order -- the ordering matters
because intermediate states leave the workspace
broken if reversed:

1. **Edit `Cargo.toml`** -- remove
   `"crates/rustbase-web"` from the workspace
   members list. Do this *first* so the next
   `cargo` invocation doesn't see a dangling member.
2. **Edit `xtask/Cargo.toml`** -- remove
   `"frontend"` from the `default` features.
   The four frontend gates leave `xtask validate`
   and its `[N/M]` count follows; no Rust edit is
   needed (see **Optional xtask features**).
   `validate` refuses to run until step 5 deletes
   `frontend/`.
3. **Edit `build.ps1`** -- prune the `frontend`
   and `e2e` subcommands plus any helpers they
   call.
4. **Edit `.claude/skills.json`** -- remove the
   `web-dev` skill entry (and any other
   frontend/E2E skill). It is *data*, not code, so
   no build step objects to a dangling `path` or a
   description advertising Axum/Svelte/Vite/
   Playwright the project no longer has -- and a
   stale skill description is exactly the kind of
   prose the compiler cannot find.
5. **Delete the now-unreferenced files**:
   - `crates/rustbase-web/`
   - `frontend/`
   - `e2e/`, `playwright.config.*`, `.ports`,
     `.ports.sample`
   - `scripts/e2e.sh`
   - `.mise.toml` (toolchain pin for Node /
     Playwright)
6. **Sweep the surviving prose.** Removing a
   subsystem strands its instructions in
   markdown, and nothing type-checks prose: grep
   `.claude/` and `CLAUDE.md` (and `llms.txt`,
   `README.md`, `docs/`) for the removed names
   (`frontend`, `e2e`, `playwright`, `svelte`,
   `vite`, `npm`) and confirm every surviving hit
   is deliberate. In practice the frontend
   assumptions in `.claude/commands/*.md` --
   `commit.md`'s `scripts/e2e.sh` step,
   `implement.md`'s frontend gates, `update-deps.md`'s
   whole npm phase -- all read as still-present until
   this sweep catches them.
7. **Run `cargo xtask validate`** to confirm the
   workspace still builds and tests pass.

`/template-sync` will then default these paths to
"skip" on future syncs since they no longer exist
locally.

## Build Commands

```bash
cargo xtask check             # type-check all targets (compiles tests, runs none)
cargo xtask validate          # every gate (see Definition of Done)
cargo xtask validate --skip <gate>  # leave a gate out (repeatable)
cargo xtask test [filter]     # tests only
cargo xtask test --ignored    # run #[ignore]-tagged tests
cargo xtask clippy            # lint (+ xtask: all features off, each alone)
cargo xtask doc               # rustdoc twice (public + private), links must resolve
cargo xtask canon-check       # claims canon prose makes: paths, xrefs, grants, width
cargo xtask records-check     # record files: ids unique and well-formed, refs resolve
cargo xtask scripts-check     # scripts/ files with a shebang staged executable
cargo xtask coverage          # coverage only (>=90%)
cargo xtask fmt               # format code
cargo xtask dupes             # code duplication check
cargo xtask audit             # security-advisory audit (RUSTSEC + npm)
cargo xtask deny              # offline licence/bans/sources gate (cargo-deny)
cargo xtask licenses          # THIRD-PARTY-LICENSES for a shipped binary
cargo xtask dep-age <eco> <pkg> [ver]  # one package's publish age
cargo xtask dep-age <eco> <pkg> --latest-aged  # newest ver past cooldown
cargo xtask dep-age-check [--base <ref>]  # cooldown-gate changed deps (vs HEAD or --base)
cargo xtask dep-preflight     # pin changed deps past cooldown pre-build
cargo xtask deploy            # deploy to remote (see docs/deployment.md)
cargo xtask deploy-setup      # one-time remote provisioning
cargo xtask backfeed-diff <ds-path>      # downstream feedback since watermark
cargo xtask backfeed-record <ds-path> --watermark <date>  # advance watermark
cargo xtask feedback-add --section <s> --title <t>  # append feedback entry
cargo xtask sync-candidates <last-synced>  # categorized sync delta, filtered
cargo xtask changelog add --kind <k> [--breaking] "text"  # insert [Unreleased] bullet
cargo xtask changelog infer-bump       # SemVer level [Unreleased] calls for
cargo xtask changelog promote --version <v> --date <d>  # date [Unreleased]
cargo xtask version-sync      # rewrite <!-- version: X --> sentinels in docs
cargo xtask verify-release-tag <tag>  # signed by a key in .github/release-signers?
cargo xtask todo <list|add|done> ...       # mechanical docs/todo.md edits
```

Never use raw `cargo test` or `cargo clippy` -- always
go through `xtask`.

The command files under `.claude/commands/` write
`cargo -q xtask ...` when they tell the agent to run one:
`-q` drops cargo's own compile progress (dozens of
`Compiling` lines after any change to `xtask/`) and leaves
the subcommand's output, stderr and exit status untouched.
Their `allowed-tools` grant both spellings.

### Frontend Development

```bash
cd frontend && npm install    # first time only
cd frontend && npm run dev    # dev server on :5173
cd frontend && npm run build  # production build to dist/
```

In dev mode, Vite proxies `/api` requests to the Axum
backend on port 3000. Run backend and frontend in
parallel:

1. `cargo run -p rustbase-web` (backend on :3000)
2. `cd frontend && npm run dev` (frontend on :5173)
3. Open http://localhost:5173

For production, build the frontend first, then serve
with the backend:
`cargo run -p rustbase-web -- --frontend frontend/dist`

**npm commands run from inside `frontend/`; `cargo xtask`
runs from anywhere.** npm must run inside `frontend/` (see
the `file:`-dependency trap below). The frontend gates find
the folder from the project root (`Config::frontend_dir` in
`xtask/src/helpers.rs`), not the shell's working directory,
so a lingering `cd frontend` does not hide them.

**A major JS bump that hits `ERESOLVE`: climb the ladder in
`/update-deps`, do not reset first.** npm compares the new
manifest against the existing tree and often emits a spurious
`ERESOLVE` (an inspector/sub-plugin the old major depended on
gets orphaned and its stale peer wins the resolve). Deleting
`frontend/node_modules` + `frontend/package-lock.json` fixes
that, but it also re-resolves every transitive to newest,
pulling many into the 14-day cooldown -- so it is the last
rung, after scoped single-package installs, a hand-edited
range and `--legacy-peer-deps`.

**Never add a `file:` npm dependency that resolves to the
repo root.** npm materializes it as a directory *junction*
on Windows (a symlink on Unix) -- e.g.
`node_modules/<root-pkg> -> <repo root>` -- which is the
reparse point a recursive delete (a worktree cleanup, `rm
-rf node_modules`) can follow straight into the main
checkout, taking `.git` with it. Such a dep also drags the
root package's whole dependency tree into the frontend
install. It resurrects unless cleared from **all three** npm
files (`package.json`, `package-lock.json`, and the hidden
`node_modules/.package-lock.json`). Related trap: run
`npm install` from **inside** `frontend/`, not
`npm --prefix frontend install` from the repo root -- the
latter resolves relative `file:` specifiers against the
caller's cwd and can re-anchor / re-create the junction.

### E2E Testing

First time on a machine, install the harness from the repo
root's lockfile (exact, cooldown-checked versions) and its
browser:

```bash
npm ci                           # root package-lock.json (Playwright)
npx playwright install chromium  # the only browser the config uses
```

The root lockfile is covered by the cooldown gate
(`dep-age-check`) but not by `cargo xtask audit`, which audits
`frontend/` only.

```bash
scripts/e2e.sh                   # free e2e ports + run tests
npx playwright test              # run all E2E tests
npx playwright test smoke        # filtered
npx playwright test --ui         # interactive UI mode
```

Playwright auto-starts its own backend and frontend on
**isolated e2e ports** (`3001` / `5174` by default), kept
separate from the dev server (`3000` / `5173`) so a run
never collides with -- or silently reuses -- a dogfooding
session. `scripts/e2e.sh` frees only those e2e ports and
leaves the dev servers running, and the e2e webServers run
with `reuseExistingServer:false` so a stale process on an
e2e port is replaced, not reused. `playwright.config.ts`
resolves the ports the same way `e2e.sh` does and pushes
them to both webServers, so a bare `npx playwright test`
(no `e2e.sh`) self-isolates identically.

**All four ports come from `.ports`** (copy from
`.ports.sample`): `backend_port` / `frontend_port` for dev,
`e2e_backend_port` / `e2e_frontend_port` for the harness;
each falls back to its default. `E2E_BACKEND_PORT` /
`E2E_FRONTEND_PORT` env vars override the e2e keys per
invocation. Because ports are machine-global, running two
worktrees -- or two rustbase-derived projects -- at once
requires giving each a **distinct four-port block** in its
own `.ports`; otherwise they collide and one project's e2e
run will free (stop) the other's server on the shared port.

**Every UI feature must have E2E tests** before the
task is marked as done. Type checking and unit tests
verify code correctness, not feature correctness.

**Stop dev servers before a full `cargo xtask validate`.**
A running dev server can starve the forked vitest workers
in the Frontend-test step (`Failed to start forks worker` /
worker-timeout), which are *environment* timeouts, not real
failures. If you see one, confirm it is a flake by
re-running `cargo xtask frontend-test` alone before treating
it as a regression.

### PowerShell Build Script

```powershell
.\build.ps1 validate      # cargo xtask validate
.\build.ps1 test          # tests only
.\build.ps1 e2e           # Playwright E2E tests
.\build.ps1 frontend      # npm build
.\build.ps1 build         # full build with all checks
.\build.ps1 deploy        # deploy to remote host
.\build.ps1 deploy-setup  # first-time remote provisioning
.\build.ps1 clean         # clean artifacts
```

## Canon vs memory

Two places hold durable guidance, and they are not
interchangeable:

- **Canon** -- this `CLAUDE.md`, `.claude/` skills and
  commands. Tracked in git, reviewed, shared across machines
  and teammates and fresh clones.
- **Memory** -- per-user auto-memory (e.g.
  `~/.claude/.../memory/`). Per-machine, never committed,
  invisible to everyone else.

**Default to canon.** A rule others would benefit from --
a workflow convention, a project constraint, a lesson from
a review -- belongs in canon. Reserve memory for genuinely
user-specific items (one operator's preferences, their
role/background, freshly-captured corrections that have not
generalized yet). When a memory entry matures into a shared
rule, promote it to canon and delete the memory copy so the
two do not drift.

## Environment Constraints

Declare machine-level assumptions here so the assistant does
not reach for tools that are not present. Fill in each
project's truths (Python, Node, Docker, cloud CLIs --
anything an assistant might invoke reflexively); name the
tool, its availability, and the allowed alternative. Example
shape:

- *(placeholder)* "Python is not installed. Do not invoke
  `python`/`python3`/`py`; use PowerShell, Bash, or Rust
  (`xtask`) for scripting." Replace with this project's
  actual constraints, or leave empty if none.

**PowerShell expands `$` inside double quotes -- use single
quotes for xtask prose.** This template targets Windows as a
first-class platform, and several `cargo xtask` commands take
prose as an argument (`changelog add`, `feedback-add --title`,
`todo add`). On Windows those arguments are usually typed into
a *double-quoted* PowerShell string, which PowerShell expands
before the program ever sees it -- and the expansion is
invisible in the diff you are about to commit, so the rule has
to be known in advance rather than learned from the symptom.
The two failure modes differ, and the quieter one is worse:

- A variable PowerShell *knows* (`$HOME`) expands to a path, so
  a real home directory lands in a tracked file (a privacy
  leak).
- One it does *not* know (`$XDG_CONFIG_HOME`) expands to the
  **empty string silently**, leaving plausible-looking but
  wrong text behind.

So text containing `$` that is passed to an xtask command must
use a **single-quoted** PowerShell string (`'...$FOO...'`) or a
Bash heredoc. Backslash does not escape `$` in PowerShell;
backtick does, which is not the reflex a Unix-trained author
has. "Avoid `$HOME`" is not enough -- every *undefined*
variable (the majority of POSIX-path mentions on Windows) is
bitten too.

## Collaboration

- **Write plainly.** One idea per sentence; lead with the
  concrete example, then the rule; prefer plain words
  ("reminder" over "forcing function", "try again" over
  "iterate"); name the subject rather than leaning on "the
  first"/"the latter". Showy phrasing looks crisp but slows
  the reader.
- **Narrate the work as it happens.** Before each meaningful
  tool call or step, say in one short sentence what is about
  to happen and why. Do not batch silently and only speak at
  the end -- a run of silent tool calls reads as "lost".
  This holds regardless of the active output style.
- **Lead with context before a decision-making question,
  and show concrete artifacts** -- for a technical choice
  (grammar, API shape, data layout), write out what each
  option looks like (side-by-side snippets / diffs) *before*
  the `AskUserQuestion`. Option labels summarize choices the
  user has already seen, not the first encounter.
- **`AskUserQuestion`: explain in layman's terms, short.**
  The lead prose must be readable by a non-expert: no
  internal type names, file paths, or API names in the
  problem statement (save those for the option
  descriptions). It states *what the decision means*, not
  *how it is implemented*.
- **Edit with `Edit`, not a slurp-mode regex, for YAML, any
  multi-line change to a `.rs` file, and anything next to a
  doc comment.** A whole-file `perl -0pi -e 's/.../.../'` has
  no idea which block it landed in. Three shapes are reliably
  dangerous. Indentation-carrying formats (YAML, TOML
  tables): a wrong-block match still parses, so it fails
  *silently* -- a substitution aimed at one workflow job
  splices into another and only surfaces steps later.
  Anything adjacent to a `///` block: inserting before an
  item reassigns the comment above it to the new item, and
  cutting an item but missing its comment leaves the comment
  glued to the next item's doc, so a function ends up
  documented as doing something else, with both rustdoc
  passes clean. And Rust source generally: a delimiter that
  also appears in the code (`|` with `||`, `{}` around a
  block) silently changes what the pattern matches.
  `sed`/`perl` remain fine for flat text and one-line
  substitutions; reach for `Edit` the moment a change spans
  lines of Rust, indentation, or a doc comment.
- **Print the variable before claiming what it holds.** A
  claim about what an environment variable, a file, or a
  platform actually contains has to arrive with the command
  that read it -- `echo $HOME`, `cat /sys/class/dmi/id/sys_vendor`,
  `gh repo view --json visibility`. The failure mode is a
  *plausible* wrong answer, not an error: "this platform does
  not expose X" is the easiest kind of statement to invent,
  nothing fails when it is wrong, and it gets written into
  several files and believed. Measure, then state.
- **Open every reply to the operator with a tagged
  summary.** Under forty words, wrapped in `[short]` and
  `[/short]` tags on their own lines, before the body. (A
  subagent's report is not such a reply; its agent file's
  format wins.) The reader reads the summary
  first and the body only when the summary sends them
  there, and the fixed, tagged place lets a hook, notifier
  or log scraper lift it out without parsing the reply. If
  the summary may be spoken, put no backticks, paths or
  symbols in it. `/short` applies the same budget on
  demand. This is an optional convention: a derived project
  that does not want it deletes this bullet.

## Coding Standards

- Rust edition 2024
- `#[deny(warnings)]` and `#[forbid(unsafe_code)]` via
  workspace lints
- Clippy pedantic where practical
- Error handling: `thiserror` for library errors,
  `anyhow` for CLI errors
- Prefer `&str` over `String` in function signatures
- All public items must have doc comments
- **Reference a private item by a backticked name, not an
  intra-doc `[link]`.** The public rustdoc pass of `cargo
  xtask doc` rejects a link from a public page to a private
  item.
- Wrap markdown at 80 characters per line
- Maximum code line width: 80 characters (`rustfmt.toml`)
- **The toolchain channel is unpinned `stable`.**
  `rust-toolchain.toml` takes the newest stable, so a new
  Clippy lint can turn the gate red with no change in the
  project (Rust 1.99's `assert_is_empty` did). Fix the
  lint, or pin the channel and bump it deliberately.

### Code comments

"All public items must have doc comments" sets a floor on
quantity; these say what goes in one. Budget: **two or three
lines** unless the comment earns more, and two things earn
more -- a list, when the thing described really is a list,
and an absence, which has to be stated and then located.

- **One reason, not the chain.** Give the reason the code
  sits where it does and stop; the reader reaches the
  consequence.
- **State the invariant, not the disaster.** "The host and
  the settings always come from the same entry" is a fact to
  rely on; "otherwise this boots on the wrong machine" reads
  as a warning to check.
- **Never inventory your callers.** That is what `grep`
  answers, and the list is stale the next time one is added.
- **No hypotheticals and no case for future work.** A
  comment describes the code as it is.
- **Say what looks wrong and is not.** A deliberate oddity
  is the comment most worth keeping.

The one reason stays in the comment. Anything longer -- the
argument behind it, the alternatives rejected -- belongs in
`docs/`, so there is one copy to keep true. A warning aimed at a
future editor rather than a reader goes there too, in an
architecture-level doc.

## Test-Driven Development

TDD is the default discipline for functional changes,
but the strict red/green ceremony applies only where
it actually produces signal. Distinguish two cases:

**Behaviour change** -- new logic in existing code, a
bug fix in shipped code, a new state transition, an
edge-case branch in a function whose other branches
already have tests:

1. **Red** -- write a failing test that describes
   the expected behaviour
2. **Green** -- write the minimal code to make the
   test pass
3. **Refactor** -- clean up while keeping tests
   green

Here the pre-implementation test failure is real
signal: it proves the test actually exercises the
new path and that the surrounding code was indeed
not already covering it. Run `cargo xtask test`
after each step to confirm the cycle.

**Structural addition** -- a new self-contained
module, a new helper function, a new enum variant
with no callers yet, a new xtask subcommand with
embedded unit tests:

Write test and implementation together as a single
unit. The whole unit lands or doesn't. Strict
red/green here is theatre: the test and impl get
written together regardless, because the unit is
too small to meaningfully fail-then-pass, and the
`unimplemented!()`-stub-first dance adds no signal.

Scope this carve-out narrowly to **pure data
declarations** -- enums/structs with derived traits
and no behaviour. The moment a "new module" or
"new helper" carries real logic (an `apply`/`inverse`,
a branch, a match), it is a behaviour change: write
the failing test first, or you will ship uncovered
branches and miss cases the after-the-fact test would
have caught.

If you're unsure which case applies, default to the
behaviour-change discipline. The cost of an
unnecessary red step is low; the cost of skipping a
real red step (and shipping a test that always
passed) is high.

## Commits and releases

**All commits must go through the `/commit` skill.**
Never use `git commit` directly. No "Co-Authored-By",
no emoji. (The exceptions are `/release`, which makes one
direct bookkeeping commit for the version bump, and the
library's release procedure in `docs/xtask-library.md`, which
does the same for `rustbase-xtask`.)

**When git has no author identity, ask for it.** Do not
offer one inferred from earlier commits as the default: past
commits may use a different address than the operator wants
on this machine.

Committing and releasing are separate:

- **`/commit`** is a save-point. It updates the
  `CHANGELOG.md` `[Unreleased]` block and commits. It does
  **no reviewing** (see **Reviewing is its own process**
  below). It does **not** bump the version, touch
  `Cargo.lock`, or run `cargo xtask validate` -- multiple
  commits land between releases, and forcing each one to
  make a SemVer decision turns the version field into
  accounting rather than a description of what users run.
  `/commit` never runs `cargo xtask validate`; run it
  manually at your own shell when you want the full gate on a
  work-in-progress.
- **`/release`** is the sole version-bumper. It infers the
  bump from the accumulated `[Unreleased]` entries
  (`**BREAKING:**` or a non-empty `### Removed` -> major,
  `### Added` -> minor, else patch; override available),
  bumps `crates/rustbase/Cargo.toml`, promotes
  `[Unreleased]` to a dated section, runs the **release
  gates** (`cargo xtask validate`, plus a **cross-platform
  clippy** on another target and a **standalone `cargo xtask
  audit`** -- because `validate` lints only the current
  platform and its audit step degrades to a *warning* when
  the advisory DB is unreachable, so "Validate OK" alone
  guarantees neither), commits the bookkeeping, and creates
  an **annotated** tag (`git tag -a vX.Y.Z`; a lightweight
  tag is invisible to the deploy guard's `git
  describe --exact-match`). When the project tracks
  `.github/release-signers`, the tag is signed (`git tag -s`)
  and checked with `cargo xtask verify-release-tag` -- only
  there: the deploy guard below does not re-check it.
- **`cargo xtask deploy`** refuses to ship unless `HEAD`
  is on a `vX.Y.Z` annotated tag matching
  `crates/rustbase/Cargo.toml` and the working tree is
  clean -- so "publish to production" is tied to "cut a
  release" by the build, not by memory. Run `/release`
  first.

### Reviewing is its own process

`/review` reviews, `/commit` commits, and neither calls the
other. Nothing requires a review: reach for `/review` when you
want work hardened before it becomes a commit, and skip it when
you do not. A project that wants a review on every commit runs
the two in sequence.

The split is deliberate. Reviewing inside a commit makes every
save-point a multi-round session -- fixes need their own
commits, and the reviewers fire again on each -- and a
save-point should be cheap.

**The reviewers get an immutable target.** Reviewing a live
working tree means reviewing something that changes while they
read it, so `/review` writes a snapshot under `target/` and
every reviewer reads that.

**Another round only when the last one fixed a behaviour
defect.** Reviewers always find something, so the stopping rule
is agreement on what matters, not a clean sheet; a round's
fixes make the next round's findings, so the count flattens
well above zero. `/review` under **What earns another round**
has the conditions and the three-round cap.

## Definition of Done

A task is done only when all of the following hold -- not
just when the code compiles:

1. **Targeted tests** for the change are written and pass.
2. **Type-check** passes (`cargo xtask check`; svelte-check
   for frontend).
3. **Browser-verify** any UI change in a real browser.
   This is the most-violated rule in practice -- treat it as
   load-bearing; "tests pass" is not "the feature works".
4. **Self-review the diff** before committing.
5. **E2E tests** for UI features (`scripts/e2e.sh`).
6. **`cargo xtask validate`** passes (the umbrella gate).

`cargo xtask validate` checks:

1. **Formatting**: auto-fixed in place by default; pass
   `cargo xtask validate --check` for the read-only
   `cargo fmt --all -- --check` (use in CI or before
   partial staging, so an in-place rewrite does not sweep
   unrelated drift into the working tree)
2. **No warnings**:
   `cargo clippy --all-targets -- -D warnings`, then
   `xtask` again with every optional feature off and
   with each one alone
3. **All tests pass**: the Test step runs the `xtask` package's
   tests, and Coverage (item 4) runs every other crate's
4. **Coverage >= 90%**, and >= 85% in each file
5. **Code duplication <= 6%** pooled across the crates
   (duplicate lines over total lines) and <= 12% in any
   one crate (production code, tests excluded). Each
   crate is measured alone, so a copy between two crates
   is not counted, by design (`docs/xtask-library.md`).

   Items 4 and 5 default to `Thresholds::default()` in
   `crates/rustbase-xtask/src/lib.rs`; a project
   overrides them in `tasks_config()`
   (`xtask/src/helpers.rs`).
6. **Frontend format** (Prettier), auto-fixed by default
7. **Frontend type check** (svelte-check)
8. **Frontend duplication** (jscpd, threshold 6%)
9. **Frontend unit tests** (vitest)
10. **Licence gate** (`cargo xtask deny`; requires
    cargo-deny) -- offline licence/bans/sources check via
    cargo-deny over `deny.toml`. A disallowed licence, a
    banned crate, or a non-crates.io source fails it. Unlike
    the audit it needs no network, so a **missing cargo-deny
    is a hard error, not a warning** -- install it with
    `cargo install --locked cargo-deny`
11. **Security audit** (RUSTSEC + npm; `cargo xtask audit`) --
    a positive vulnerability fails; an unreachable advisory
    DB degrades to a warning
12. **Dependency cooldown** (`cargo xtask dep-age-check`) --
    fails when a dependency added or bumped since `HEAD` was
    published within the 14-day window, or when its age cannot
    be checked (never a warning); unchanged lockfiles make it a
    no-op
13. **Canon claims** (`cargo xtask canon-check`) -- reads
    `CLAUDE.md`, `.claude/commands/` and `.claude/agents/`
    and fails on any of five things:
    - a backticked path under `crates/`, `xtask/`, `docs/`,
      `scripts/` or `.claude/` that does not exist;
    - a bold cross-reference introduced by "under" that names
      no heading and no bold lead (a `**...**` run opening a
      line);
    - in a command file with an `allowed-tools` line, an
      inline backticked `git` command no grant covers as a
      word prefix -- checked inside a prose quotation too, and
      per command of a chain. A file that only prints one for
      the operator says it has "no `git <sub>` grant", which
      exempts that subcommand file-wide;
    - prose past 80 columns;
    - a backlog ID (`rt-`/`aq-`/`fr-`) cited in no backlog
      (this check also reads `docs/`, except the reviewer
      logs, `template-feedback.md` and `docs/issues/`).

    Not seen: root files; path globs, `<placeholder>` paths
    and paths ending in `/`; paths a project exempts in
    `tasks_config()` (`xtask/src/helpers.rs`, each with a
    reason); backlog IDs in the `docs/` files excepted
    above; `git` in fenced
    blocks, in spans that wrap across lines, or after `$(`
    or a `VAR=` prefix (rt-2026-10-19-canon-git-check-gaps);
    width in tables, fences, frontmatter and 4-space-indented
    lines; `.claude/skills/` and the rest of `docs/`. A green
    gate says nothing about these
14. **Record files** (`cargo xtask records-check`) -- reads
    `docs/todo.md`, the three `docs/developer/*-log.md`
    reviewer logs and `docs/developer/template-feedback.md`
    (`Config::record_files`, overridden in `tasks_config()`,
    `xtask/src/helpers.rs`). Each entry opens with a `### <id>`
    heading; its fields are the `**Label:**` lines directly
    under it, and bold lines further down are prose. Fails on
    an id used twice across the files, an id that is not
    kebab-case starting with a letter (so a bare date is
    refused), a field label outside the known
    set (`Category`, `Summary`, `Source`, `Issue`, `Depends
    on`, `Supersedes`), and a `Depends on` or `Supersedes` id
    that names no entry. An id cited in prose is not checked,
    and a configured file that does not exist is skipped and
    named in the result
15. **Executable scripts** (`cargo xtask scripts-check`) --
    every file under `scripts/` that starts with `#!` must be
    staged as mode `100755` (the git index), since canon
    runs them by bare path (`/commit` runs `scripts/e2e.sh`)
    and the shell refuses a `100644` one with "permission
    denied" (exit 126). A
    file with no shebang (a sourced library) is exempt. With no
    `.git` in the project folder or above it (a source tarball)
    the gate reports itself skipped; inside a work tree any git
    failure fails it
16. **Doc links** (`cargo xtask doc`) -- rustdoc with
    `-D warnings`, twice: once public and once with
    `--document-private-items`, because each pass sees a
    class of broken link the other cannot

The frontend gates skip only when there is no frontend at
all. Without xtask's `frontend` feature they are not compiled
in, and `validate` refuses to run while `frontend/` exists; it
refuses too when `tasks_config()` (`xtask/src/helpers.rs`) sets
no `frontend_dir`, which would make them skip.
The dependency-cooldown gate runs **first**: it is a no-op on
unchanged lockfiles, and fails fast on a within-cooldown
dependency before anything compiles it. The static gates follow,
cheapest first: Fmt, Canon, Records, Scripts, Duplication,
Clippy, Doc, Deny, then the frontend static gates. The dynamic
gates come last: the frontend and Rust tests, Coverage, then
Audit, which needs the network. A failed step prints the single
command to re-run just that gate. `validate --skip <gate>` leaves a
gate out by name or subcommand and says so before the first step
and after the closing `Validate OK`; skipping Coverage still runs
every crate's tests, in a step in its place. On a
fresh full-stack checkout, run
`npm install` inside `frontend/` before `validate` -- the
frontend gates fail (rather than silently skipping) when
`frontend/` exists but its `node_modules` is not
installed.

## Semantic Versioning

Follow [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** -- breaking changes
- **MINOR** -- new features, backwards-compatible
- **PATCH** -- bug fixes, documentation, internal refactors

The version lives in `crates/rustbase/Cargo.toml` and is
the **single source of truth**. `/release` is the only thing
that changes it; it computes the bump from the accumulated
`[Unreleased]` CHANGELOG entries (see "Commits and
releases").

## Release Notes

Maintain `CHANGELOG.md` using the
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
format. Group changes under: **Added**, **Changed**,
**Fixed**, **Removed**.

Always keep an `[Unreleased]` section at the top. `/commit`
appends bullets there (marking breaking changes with a
leading `**BREAKING:**`); `/release` promotes the whole
block to a dated `## [X.Y.Z] - YYYY-MM-DD` section and opens
a fresh empty `[Unreleased]` above it.

## Skills

| Skill | Purpose |
|-------|---------|
| `/check` | Type-check all targets (compiles tests, runs none) |
| `/test` | Run tests with agent-friendly output |
| `/validate` | Full quality pipeline with stepwise progress |
| `/commit` | Save-point commit with CHANGELOG entry (no review, no version bump) |
| `/review` | Three reviewers in sequence, one lane each, against an immutable snapshot; commits nothing |
| `/issue` | Work a GitHub issue end to end: verify, plan, TDD, review, PR (needs `gh`) |
| `/ask` | Re-put the last decision raised in prose as a structured question |
| `/rundown` | Grouped one-line rundown of the session, ending with what is open |
| `/short` | Restate the reply above in under 40 words |
| `/release` | Cut a SemVer release: bump the version, promote `[Unreleased]`, validate, commit, and tag |
| `/retrospect` | Workflow retrospective (Efficiency / Quality / Speed / Cleanup). Invoked automatically by `/commit`; also callable manually mid-session |
| `/todo` | Capture a work item into `docs/todo.md` (no implementation) |
| `/implement` | Plan + implement a captured item; keeps a working `docs/issues/<slug>.md` until done |
| `/update-deps` | Upgrade third-party deps (Rust + frontend) to the newest versions outside the 14-day cooldown |
| `/simplify` | Review changed code for quality |
| `/architect` | Project overview and architecture guide |
| `/web-dev` | Axum, Svelte 5, Vite, Playwright patterns |
| `/html-report` | Produce a self-contained local HTML report from the in-repo template (never a cloud Artifact) |
| `/template-improve` | Log feedback for the rustbase template |
| `/template-sync` | Sync upstream template changes |
| `/template-backfeed` | Apply downstream feedback back into this template (template repo only) |

## Template tooling: determinism vs judgment

The template-maintenance workflows (`/template-sync`,
`/template-backfeed`, `/template-improve`) split their work
into two kinds, and the split is load-bearing:

- **Determinism -- belongs in `cargo xtask`.** Delta
  determination (what changed since a watermark / SHA), log
  bookkeeping (appending an entry, minting an ID, dedup), and
  exclude-set filtering are mechanical. They must run as
  unit-tested `cargo xtask` commands, never as an LLM scan of
  a growing markdown file. An LLM re-reading a 2000-line log
  on every run is unbounded cost and drifts on format.
- **Judgment -- belongs to the LLM.** Categorizing a change,
  deciding apply/skip, merging code, writing prose. This is
  what the commands hand back to the agent.

Concretely: `backfeed-diff` (delta since the ledger
watermark), `backfeed-record` (advance the watermark),
`feedback-add` (append with a `tf-<date>-<slug>` ID), and
`sync-candidates` (categorized diff minus the never-sync set)
own the determinism; the slash commands own the judgment. When
extending these workflows, keep new mechanical work in xtask
with tests -- do not push it back into the prompt.

## Template Sync

This project tracks its template origin in
`.template-sync.toml`. Use `/template-sync` to pull
improvements from the upstream
[rustbase](https://github.com/breki/rustbase) template.
The command fetches upstream changes, then calls
`cargo xtask sync-candidates` to get a categorized file
delta with template-internal bookkeeping files already
filtered out, and helps you selectively apply relevant
updates while preserving your project's customizations.

## Template Feedback

This project was generated from the
[rustbase](https://github.com/breki/rustbase) template.
When you notice anything in the template-provided files
that is suboptimal, incorrect, outdated, or could be
improved, log it in `docs/developer/template-feedback.md`.

Examples of what to log:
- Dependency versions that needed immediate updating
- Config that didn't work out of the box
- Patterns that had to be reworked early on
- Missing features that every project ends up adding
- Conventions that turned out to be impractical
- Unnecessary boilerplate that was deleted

This feedback will be used to improve the template for
future projects.

The file uses three sections (see its header for
section semantics): **Open divergences** (gaps the
project intentionally keeps), **Resolved** (gaps closed
by retrofit work), and **Suggestions to flow back to
the template**. `/template-improve` routes new entries
into the appropriate section by calling
`cargo xtask feedback-add`, which mints a stable
`tf-<yyyy-mm-dd>-<slug>` ID, inserts at the section top,
and dedups. Whether the file may be edited by hand is
`/template-improve`'s rule to state.

`/template-backfeed` (template repo only) pulls a
downstream's feedback back upstream. It uses a watermark in
`docs/developer/backfeed-ledger.toml` (one table per
downstream, machine-owned by `cargo xtask backfeed-record`)
so each run evaluates only feedback newer than the last, via
`cargo xtask backfeed-diff` -- it never re-scans the whole
downstream file.

## Optional xtask features

`xtask` groups its optional commands into Cargo features,
all on by default:

| Feature | Commands |
|-|-|
| `frontend` | the four `frontend-*` gates and commands, from the library's `frontend` feature |
| `deploy` | `deploy`, `deploy-setup` |
| `template-tools` | `sync-candidates`, `feedback-add`, `backfeed-*` (`/template-sync`, `/template-improve`, `/template-backfeed`) |
| `clean-cache` | `clean-cache` |

A project drops a group by removing it from `default` in
`xtask/Cargo.toml`. The group's modules, subcommands and
`validate` steps compile out together, and `[N/M]` counts
only the steps left. Each group's helpers live in its own
modules, so nothing is left behind as dead code. A derived
project keeps `template-tools`: `/template-sync` and
`/template-improve` run its commands.

`cargo xtask clippy` checks that no group leaves dead code
behind: after the workspace pass it lints `xtask` with every
feature off and with each feature alone. A helper that only
one group uses but that sits outside it fails the all-off
pass. A group that needs another fails when it runs alone.

A shared gate goes into the library's list,
`step::standard` (`crates/rustbase-xtask/src/step.rs`), which
also fixes the order, and its subcommand into `cli::LibCommand`
(`crates/rustbase-xtask/src/cli.rs`), which `xtask`'s
`XCommand` flattens in. A shared gate behind a library feature
stays out of that list: it offers its own `steps()`, which the
project places, as `xtask/src/validate.rs` places
`frontend::steps` after Deny. A project's own gate is a `step()`
function in its own `xtask` module returning a `Step`
(`Step::new(name, cmd, closure)`), with its own `XCommand`
variant; `xtask/src/validate.rs` places it with
`step::insert_after(&mut steps, step::names::DENY, [...])`, after
Deny for a static gate. An optional gate also takes
`#[cfg(feature = "<name>")]` in each place that names it:

- the feature in `[features]` of `xtask/Cargo.toml`;
- its `mod` line in `xtask/src/main.rs`;
- its `XCommand` variant and `match` arm there;
- its entry, and its `use`, in `xtask/src/validate.rs`.

## Workspace lints and xtask overrides

The workspace forbids `unsafe_code` via
`[workspace.lints.rust]` so production crates inherit
the policy by default. If a derived project needs OS-
specific code in `xtask/` (for example, calling Win32
APIs for process management on Windows -- the canonical
case being `OpenProcess` / `TerminateProcess` /
`CreateToolhelp32Snapshot` for stale-server cleanup),
the recipe is to redefine the lints block locally for
`xtask` only rather than weakening the workspace policy:

```toml
# xtask/Cargo.toml
[lints.rust]
warnings = "deny"
unsafe_code = "allow"   # xtask is build tooling, scoped exception

[lints.clippy]
# inherit the workspace clippy block by re-declaring
# or by overriding selectively
```

Production crates keep `[lints] workspace = true` and
remain `unsafe`-forbidden. Document the scoped
exception with a comment near the use site so reviewers
can verify the unsafe block is genuinely necessary.

## Coverage exceptions for hardware-bound code

The 90% coverage gate (see Definition of Done) assumes
every code path can run under `cargo llvm-cov` in CI.
Real projects routinely have I/O paths that can't:
audio playback, network calls against external
services, native API calls (Win32, CoreAudio, ALSA),
GPIO on embedded targets. The recipe for keeping the
gate honest without weakening it:

1. **Extract the hardware-bound code into a sibling
   submodule.** Given `foo.rs` that contains both
   business logic and an I/O call, split into `foo.rs`
   (the orchestrator) and `foo/bar.rs` (the I/O leaf).
   The leaf module should be as small as possible --
   ideally just the unmockable call plus its
   immediate error mapping.
2. **Exclude the leaf submodule via manifest config.**
   Add its path (a regex fragment) to
   `[workspace.metadata.coverage] ignore` in the **root
   `Cargo.toml`** -- no need to fork `xtask`:

   ```toml
   [workspace.metadata.coverage]
   # Each entry is a regex fragment merged into the coverage
   # --ignore-filename-regex baseline. Use single-quoted TOML
   # literal strings so backslashes reach the regex verbatim
   # (no doubling).
   ignore = ['src[/\\]audio[/\\]playback\.rs']
   ```

   `cargo xtask coverage` merges these with its built-in
   baseline (`src/main.rs`, `src/bin/`); the leaf module is
   exempted from the gate, the orchestrator is not. An absent
   section leaves the baseline unchanged, but a
   missing/unreadable manifest fails the gate: the baseline
   alone would report the excluded files as a coverage drop.
   A blank entry is ignored; a pattern that would match
   *every* file (`.`, `.*`, `.+`, `.*?`) is rejected; and a
   run that measured no files fails whatever pattern caused
   it -- either would silently neuter the gate. Only the
   `[workspace.metadata.coverage]` + line-leading
   `ignore = [...]` shape is read; the dotted-key
   (`coverage.ignore = ...`) and inline-table spellings are
   not.
3. **Add a `*_TEST_*` env-var escape hatch in the
   excluded module.** For example, `RUSTBASE_TEST_AUDIO`
   short-circuits the real native call and returns a
   fixed `Ok`/`Err` shape. This keeps the parent
   module's post-call success and error branches
   testable -- they're the parts that actually carry
   business logic, and they remain inside the 90% gate.

What this gets you: the orchestrator is fully covered
(including both branches of its `match
play_audio_native() { Ok => ..., Err => ... }`), the
leaf is honestly acknowledged as untested in CI, and
there's no `#[cfg(test)]` test-only branch leaking into
production code paths.

When NOT to use this recipe: if the I/O can be faked
with a trait + dependency injection at the call site
without contortions, do that instead. The submodule-
plus-ignore-regex pattern is for cases where the
indirection itself would obscure the code more than it
reveals.

## Shell wrappers: bash and PowerShell twins

This template targets Windows, Linux, and macOS as
first-class platforms. The convention for cross-shell
tooling is: **non-trivial logic lives in `cargo
xtask`; shell files (`scripts/*.sh`, `*.ps1`) are
thin wrappers only.** This keeps a bugfix from having
to land twice in two languages whose semantics drift
(quoting, exit codes, error handling).

The canonical wrapper shapes are:

```bash
# scripts/foo.sh
#!/usr/bin/env bash
set -euo pipefail
exec cargo xtask foo -- "$@"
```

```powershell
# scripts/foo.ps1
$ErrorActionPreference = 'Stop'
& cargo xtask foo -- @args
exit $LASTEXITCODE
```

Exceptions are allowed where the logic genuinely
can't live in Rust without contortion -- e.g.
process-cleanup that pokes `Get-CimInstance` or
`pkill` directly, or bootstrap scripts that run
*before* `cargo` is available. Document such
exceptions inline so the next reader knows why the
file is not a wrapper.

## Long-running scripts

For any script that runs more than ~30 seconds
(`scripts/e2e.sh`, dogfood/deploy helpers):

- **Author side** -- tee stdout to `target/<name>.log` so
  the output is durable (a captured caller, CI, or a closed
  terminal otherwise loses it). With the
  `exec > >(tee "$LOG") 2>&1` idiom you must also capture
  `TEE_PID=$!` and `wait "$TEE_PID"` in the `EXIT` trap --
  bash does not synchronize with `>(...)` process
  substitution on exit, so the trailing trap output (often
  the most important lines) is silently truncated without
  the wait. A `cargo xtask` command needs no shell `tee`, and
  so has no wait problem: `tee::Tee` in `crates/rustbase-xtask`
  gives it the same copy in `target/<name>.log`. `validate`
  uses it for
  `target/validate.log` (its step lines and the failure,
  rewritten each run; a gate's own warnings print to the
  terminal only). A second `validate` in the same checkout is
  refused while the first holds the log. One that cannot open
  it for another reason warns and runs unlogged, and the file
  then still holds an earlier run's output.
- **Caller side** -- **never pipe a long-running command
  through `tail -N` under a tight timeout.** `tail -N` says
  "give me the end"; the timeout says "there will be no
  end" -- it buffers until EOF that never comes within the
  window, so the pipeline shows nothing and reads as a
  stall. Use `run_in_background` for the completion
  notification, or a `Monitor` with a line-buffered grep for
  progress; reserve `| tail -N` for already-finished
  commands.
- **A background wrapper must exit with the command's
  status.** `cmd > log; echo "exit=$?" >> log` records the
  status, then exits 0 itself, so the completion
  notification reports success while the command failed.
  End such a wrapper with `exit` on the saved status, or
  read the log's `exit=` line before trusting the
  notification.

## Lints: `doc_markdown` allowlist via `clippy.toml`

The workspace runs clippy with pedantic lints enabled
where practical. `clippy::doc_markdown` flags
identifiers like `PowerShell`, `JSON`, `FFI`,
`WebSocket`, `macOS`, `GitHub` in doc comments,
forcing every occurrence to be backticked even when
the prose reads naturally without backticks.

The template ships a `clippy.toml` at workspace root
with a curated `doc-valid-idents` allowlist of
infrastructure terms. The list extends clippy's
defaults (via the `".."` sentinel as the first entry)
rather than replacing them. Derived projects should
**append** their own domain-specific identifiers
(product names, acronyms, external systems) to that
file rather than redefining the list.

## Edition-2024 migration notes

The template ships on Rust edition 2024. Projects
inheriting from an older snapshot of the template (or
upgrading from edition 2021) routinely hit a small set
of mechanical fixes that `cargo fix --edition` either
applies automatically or flags:

- **Unsafe extern blocks**: `extern "C" { fn foo(); }`
  must become `unsafe extern "C" { fn foo(); }`. Each
  declaration inside is still individually `unsafe fn`.
- **Match ergonomics tightening**: bare `ref` patterns
  inside a binding that already implies a reference
  must be dropped. `match x { Some(ref y) => ... }`
  becomes `match x { Some(y) => ... }` when the outer
  match already produces a reference.
- **`gen` is reserved**: any identifier called `gen`
  (variables, function names, struct fields) needs the
  raw-identifier form `r#gen` or a rename.
- **Nested `if let` -> let chains**: clippy's autofix
  collapses `if x { if y { ... } }` into
  `if x && y { ... }` once `let`-chains are stable.
  This is a clippy fix rather than an edition fix, but
  it lands at the same time and is worth running in the
  same pass.

Run `cargo fix --edition --workspace` followed by
`cargo xtask validate` and expect a small follow-up
pass for the items above.

## Version source of truth

The project version lives in
`crates/<name>/Cargo.toml`. Avoid putting the version
number in README body text or other markdown — those
copies drift silently from `Cargo.toml`. If a version
mention is unavoidable, either pull the value from
`Cargo.toml` via the build (Vite supports this for the
frontend; CLI binaries can use `env!("CARGO_PKG_VERSION")`),
or let `cargo xtask version-sync`, which `/release` runs,
rewrite it in `README.md` or a doc under `docs/`.

For the second, the file needs a sentinel comment on a line
of its own, outside any code block: that line opts the file
in. The command then rewrites the sentinel and the number
in every `VERSION=<number>` line in the file:

````md
<!-- version: 0.5.0 -->

```bash
VERSION=0.5.0
```
````

It touches nothing else, so a version in a sentence still
drifts.

## Supply-chain hygiene

Five `cargo xtask` commands guard the dependency tree:

- **`cargo xtask deny`** runs `cargo deny check licenses bans
  sources` over `Cargo.lock` and the on-disk crate metadata,
  configured by `deny.toml` at the workspace root. It is
  **offline** -- it reaches no network -- which is what lets it
  run on every push in CI as well as in `validate`, where the
  advisory `audit` cannot. A disallowed licence, a banned crate,
  or a non-crates.io source fails it. Because there is no network
  to be down, a **missing `cargo-deny` is a hard error**, not a
  degrade-to-warning like `audit` (`cargo install --locked
  cargo-deny`). The allow-list in `deny.toml` is meant to match
  the tree exactly (`unused-allowed-license = "deny"`), so after
  adding a dependency, run `cargo xtask deny` and adjust the
  list as it reports. Note that `deny` does **not** exempt
  `publish = false` crates, so every workspace member carries a
  `license` field.
- **`cargo xtask audit`** runs `cargo audit` (RUSTSEC) over
  `Cargo.lock` and `npm audit` over the frontend, failing on
  any vulnerability (advisory *warnings* -- unsound /
  unmaintained / yanked -- are reported, not fatal). The failure
  lists each vulnerability on a line of its own: the RUSTSEC id,
  crate, version, title and patched ranges, or the npm package,
  affected range, severity, advisory and fix. It needs
  `cargo-audit` (`cargo install cargo-audit`) and network access
  to the advisory DB / npm registry. Without either, the
  standalone `cargo xtask audit` **fails**, but inside `validate`
  (which runs it late) it only **warns** and the gate passes --
  which is why `/release` also runs the standalone command.
- **`cargo xtask dep-age <npm|cargo> <package> [version]`**
  reports how many days ago a version was published (on-demand,
  a single package). Add **`--latest-aged`** to instead print
  the **highest** version that has cleared the cooldown
  (selected by version, not publish date) -- the pin target the
  `/update-deps` workflow feeds to `cargo update --precise` /
  `npm install <pkg>@<ver>`.
- **`cargo xtask dep-age-check`** enforces the cooldown as the
  **first** `validate` step, so a dependency adopted within the
  cooldown fails the gate before the compile steps (Clippy,
  Test, Coverage) build and run its build script. It checks
  **only the dependencies added or version-bumped in the working
  tree versus `HEAD`** (`Cargo.lock`, `frontend/package-lock.json`
  and the root `package-lock.json` of the E2E harness; a lockfile
  absent at the base counts as wholly new), so it fires exactly when
  a dependency is adopted and costs nothing -- no network -- on a
  commit that leaves the lockfiles untouched. A *whole-tree* gate is
  deliberately avoided: it would flag every already-locked
  version on every routine update. A check that cannot finish
  (unreachable registry, missing `HEAD` baseline, an unreadable or
  unparseable lockfile) **fails**, inside `validate` as well as
  standalone: unlike `audit`, it never degrades to a warning,
  because a dependency whose age could not be checked is exactly
  the one a flaky connection would wave through. Offline work is
  not blocked by it: an unchanged lockfile needs no network. With
  no `HEAD` at all (before the first commit, or a source tarball)
  there is nothing to diff against and no allow-list entry can
  help: commit first, or run `cargo xtask validate --skip dep-age`.
  **CI runs the standalone command** (the `dep-age` job in
  `.github/workflows/ci.yml`) with `--base` set to the commit the
  pull request or push started from -- in CI the checkout *is*
  `HEAD`, so diffing against `HEAD` would always pass.
- **`cargo xtask dep-preflight`** is the *pre-compile* twin of
  `dep-age-check`. Where the gate reports a cooldown breach
  *after* the fact, preflight *remediates* it *before* you
  build: it reads the changed Rust crates (same `HEAD` diff as
  the gate) and, for each one still inside the cooldown, pins
  it down to its newest aged version with
  `cargo update --precise`, looping until the whole changed set
  is aged or no aged version fits the resolved requirements.
  Every step touches only the registry index and the lockfile,
  so no crate tarball is fetched and no build script runs until
  the tree is clean. Use it as a front door: `cargo add <dep>`
  (updates the lockfile, no compile) -> `cargo xtask
  dep-preflight` -> `cargo build`. Rust / crates.io only.

**Why both a gate *and* a preflight?** `dep-age-check` is a
*post-resolution* check -- by the time it fails, `cargo` has
already downloaded *and compiled* the fresh crates, running
their build scripts (`cc`, `ring`, ...) on your machine. The
gate protects the committed lockfile (and everyone who builds
from it); it does not protect the build host during the window.
`dep-preflight` closes that host-side gap, but only when you
run it *instead of* going straight to `cargo build` -- it
cannot intercept a bare `cargo build` that resolves and
compiles in one shot. The only thing that protects *every*
invocation automatically is cargo's in-resolver
**`-Zmin-publish-age`** (RFC 3923), which refuses to *select* a
too-new version so it is never fetched or built. That flag's
client side is nightly-only as of now; once it stabilizes on
stable, layer it in front of (or in place of) these xtask
commands. Until then, `dep-age-check` is the CI-enforced gate
(runs on stable) and `dep-preflight` is the opt-in host-side
hardening. A downstream project points to the third-party
`cargo-cooldown` as a stable-today, fetch-time stopgap; it is
not evaluated here, and it is itself new code running on the
build host, so put it through the same review and cooldown as
any other tool before adopting it.

**Dependency-version cooldown.** Do not adopt a dependency
version published fewer than 14 days ago without a stated
justification -- that window is when a compromised or
malicious release is most likely still live. Security fixes
are exempt (the fix's urgency outweighs the cooldown). Check
a candidate before adding it:
`cargo xtask dep-age cargo <crate> <version>` (or `npm`); it
exits non-zero when the version is within the cooldown.

`validate` enforces this automatically for changed deps via
the `dep-age-check` step above. When you *do* adopt a fresh
version with justification (or a security fix), waive it in
**`.dep-age-allow`** at the root, one line per version, the
reason as a comment:

```
ring@0.17.9   # RUSTSEC-2026-0042; the only fix is 3 days old
```

The gate reads that file **as committed at the base it diffs
against** -- `HEAD` locally, the pull request's base in CI, or
the tip before a push to `main` -- so the waiver lands on the
base branch first, as its own push or merged pull request, and
the change adopting the version is pushed after it. A waiver in
the same change waives nothing, and one push of both fails; a
pull request opened earlier needs a new push, since a re-run
keeps the old base. Being a file, every waiver stays on record
in git; prune a line once its version clears the cooldown. A
malformed line never fails the gate (a typo merged to `main`
must not block every later change, including its fix); it is
printed with any failing run's message instead.

For a run before the waiver is committed, the
**`RUSTBASE_DEP_AGE_ALLOW`** env var (`name@version`,
comma-separated) waives locally -- e.g.
`RUSTBASE_DEP_AGE_ALLOW=serde@1.0.999 cargo xtask validate` --
and CI also reads it from the maintainer-set repository variable
of that name (Settings, then Variables).

**`cargo update` interaction.** The gate checks *every*
newly-locked registry dependency, **transitive ones included**
-- so it is a no-op only on commits that leave the lockfiles
untouched, not on every "routine" commit. A lockfile-churning
update (`cargo update`, `npm update`) can bump many transitive
crates to versions published within the cooldown, and the gate
will fail listing all of them. That is intended -- a bulk
update is exactly when a freshly-published (possibly
compromised) transitive release slips in. The recommended
workflow: run the update, then either wait out the cooldown
before committing, or, once you've reviewed the flagged
versions, bulk-approve them: one `.dep-age-allow` line each,
landed first as above (for the local run before then,
`RUSTBASE_DEP_AGE_ALLOW=a@1.2.3,b@4.5.6,... cargo xtask
validate`). Prefer scoped updates (`cargo update -p <crate>`)
over a blanket `cargo update` so the flagged set stays small
and reviewable.
