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
| `xtask` | Build automation |

The web crate is optional. For a CLI-only project,
follow these steps in order -- the ordering matters
because intermediate states leave the workspace
broken if reversed:

1. **Edit `Cargo.toml`** -- remove
   `"crates/rustbase-web"` from the workspace
   members list. Do this *first* so the next
   `cargo` invocation doesn't see a dangling member.
2. **Edit `xtask/src/validate.rs`** (and any wiring
   in `xtask/src/main.rs`) -- drop the
   `svelte-check` step and any other
   frontend/E2E-specific steps from `xtask
   validate`. Adjust the step counter
   (`[N/M] ...`) so the progress output stays
   accurate.
3. **Edit `build.ps1`** -- prune the `frontend`
   and `e2e` subcommands plus any helpers they
   call.
4. **Delete the now-unreferenced files**:
   - `crates/rustbase-web/`
   - `frontend/`
   - `e2e/`, `playwright.config.*`, `.ports`,
     `.ports.sample`
   - `scripts/e2e.sh`
   - `.mise.toml` (toolchain pin for Node /
     Playwright)
5. **Run `cargo xtask validate`** to confirm the
   workspace still builds and tests pass.

`/template-sync` will then default these paths to
"skip" on future syncs since they no longer exist
locally.

## Build Commands

```bash
cargo xtask check             # fast compile check
cargo xtask validate          # fmt + clippy + tests + coverage
cargo xtask test [filter]     # tests only
cargo xtask test --ignored    # run #[ignore]-tagged tests
cargo xtask clippy            # lint only
cargo xtask coverage          # coverage only (>=90%)
cargo xtask fmt               # format code
cargo xtask dupes             # code duplication check
cargo xtask deploy            # deploy to remote (see docs/deployment.md)
cargo xtask deploy-setup      # one-time remote provisioning
```

Never use raw `cargo test` or `cargo clippy` -- always
go through `xtask`.

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

**Before bumping a major version of a JS dep, delete
`frontend/node_modules` + `frontend/package-lock.json`
first**, then `npm --prefix frontend install`. npm compares
the new manifest against the existing tree and frequently
emits a spurious `ERESOLVE` on an otherwise-clean upgrade
(typically an inspector/sub-plugin the old major depended
on gets orphaned and its stale peer wins the resolve). A
from-scratch resolve avoids it. Cheap to do unconditionally
for a major bump; expensive to diagnose after the fact.

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

## Coding Standards

- Rust edition 2024
- `#[deny(warnings)]` and `#[forbid(unsafe_code)]` via
  workspace lints
- Clippy pedantic where practical
- Error handling: `thiserror` for library errors,
  `anyhow` for CLI errors
- Prefer `&str` over `String` in function signatures
- All public items must have doc comments
- Wrap markdown at 80 characters per line
- Maximum code line width: 80 characters (`rustfmt.toml`)

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

## Commits

**All commits must go through the `/commit` skill.**
Never use `git commit` directly. No "Co-Authored-By",
no emoji.

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
   `cargo clippy --all-targets -- -D warnings`
3. **All tests pass**: `cargo test`
4. **Coverage >= 90%**
5. **Code duplication <= 6%** (production code, tests
   excluded)
6. **Frontend format** (Prettier), auto-fixed by default
7. **Frontend type check** (svelte-check)
8. **Frontend duplication** (jscpd, threshold 6%)
9. **Frontend unit tests** (vitest)

The frontend gates (6-9) skip only when there is no
frontend at all. Gates run cheapest-first (Fmt,
Duplication, Clippy, then the frontend static gates) before
the expensive dynamic gates (frontend + Rust tests,
Coverage), and a failed step prints the single command to
re-run just that gate. On a fresh full-stack checkout, run
`npm --prefix frontend install` before `validate` -- the
frontend gates fail (rather than silently skipping) when
`frontend/` exists but its `node_modules` is not
installed.

## Semantic Versioning

Follow [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** -- breaking changes
- **MINOR** -- new features, backwards-compatible
- **PATCH** -- bug fixes, documentation, internal refactors

The version lives in `crates/rustbase/Cargo.toml` and is
the **single source of truth**.

## Release Notes

Maintain `CHANGELOG.md` using the
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
format. Group changes under: **Added**, **Changed**,
**Fixed**, **Removed**.

Always keep an `[Unreleased]` section at the top.

## Skills

| Skill | Purpose |
|-------|---------|
| `/check` | Fast compilation check (no tests) |
| `/test` | Run tests with agent-friendly output |
| `/validate` | Full quality pipeline with stepwise progress |
| `/commit` | Commit with versioning, diary, and code review |
| `/retrospect` | Workflow retrospective (Efficiency / Quality / Speed). Invoked automatically by `/commit`; also callable manually mid-session |
| `/todo` | Capture a work item into `docs/todo.md` (no implementation) |
| `/implement` | Plan + implement a captured item; writes `docs/issues/<slug>.md` |
| `/simplify` | Review changed code for quality |
| `/architect` | Project overview and architecture guide |
| `/web-dev` | Axum, Svelte 5, Vite, Playwright patterns |
| `/template-improve` | Log feedback for the rustbase template |
| `/template-sync` | Sync upstream template changes |
| `/template-backfeed` | Apply downstream feedback back into this template (template repo only) |

## Template Sync

This project tracks its template origin in
`.template-sync.toml`. Use `/template-sync` to pull
improvements from the upstream
[rustbase](https://github.com/breki/rustbase) template.
The command fetches upstream changes, categorizes them,
and helps you selectively apply relevant updates while
preserving your project's customizations.

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
into the appropriate section.

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
2. **Add the leaf submodule to the coverage
   `IGNORE_REGEX`** in `xtask/src/coverage.rs`. The
   existing default excludes `src/main.rs` only; extend
   it with the new path. The leaf module is exempted
   from the gate; the orchestrator is not.
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
  the wait.
- **Caller side** -- **never pipe a long-running command
  through `tail -N` under a tight timeout.** `tail -N` says
  "give me the end"; the timeout says "there will be no
  end" -- it buffers until EOF that never comes within the
  window, so the pipeline shows nothing and reads as a
  stall. Use `run_in_background` for the completion
  notification, or a `Monitor` with a line-buffered grep for
  progress; reserve `| tail -N` for already-finished
  commands.

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
mention is unavoidable in user-facing prose, embed it
as a sentinel comment (`<!-- version: 0.5.0 -->`) so a
script can rewrite both on release, or pull the value
from `Cargo.toml` via the build (Vite supports this
for the frontend; CLI binaries can use `env!("CARGO_PKG_VERSION")`).

## Supply-chain hygiene

Two `cargo xtask` commands guard the dependency tree:

- **`cargo xtask audit`** runs `cargo audit` (RUSTSEC) over
  `Cargo.lock` and `npm audit` over the frontend, failing on
  any vulnerability (advisory *warnings* -- unsound /
  unmaintained / yanked -- are reported, not fatal). It runs
  as the final `validate` step, so **`validate` needs
  `cargo-audit` installed (`cargo install cargo-audit`) and
  network access** to the advisory DB / npm registry.
- **`cargo xtask dep-age <npm|cargo> <package> [version]`**
  reports how many days ago a version was published.

**Dependency-version cooldown.** Do not adopt a dependency
version published fewer than 14 days ago without a stated
justification -- that window is when a compromised or
malicious release is most likely still live. Security fixes
are exempt (the fix's urgency outweighs the cooldown). Check
a candidate before adding it:
`cargo xtask dep-age cargo <crate> <version>` (or `npm`); it
exits non-zero when the version is within the cooldown.
