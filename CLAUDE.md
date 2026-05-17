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

The web crate is optional. To remove it: delete
`crates/rustbase-web/`, `frontend/`, and remove
`"crates/rustbase-web"` from `Cargo.toml` workspace
members.

## Build Commands

```bash
cargo xtask check             # fast compile check
cargo xtask validate          # fmt + clippy + tests + coverage
cargo xtask test [filter]     # tests only
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

### E2E Testing

```bash
scripts/e2e.sh                   # kill stale servers + run tests
npx playwright test              # run all E2E tests
npx playwright test smoke        # filtered
npx playwright test --ui         # interactive UI mode
```

Playwright auto-starts both backend and frontend.
Configure ports via `.ports` file (copy from
`.ports.sample`).

**Every UI feature must have E2E tests** before the
task is marked as done. Type checking and unit tests
verify code correctness, not feature correctness.

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

Follow red/green TDD for all functional changes:

1. **Red** -- write a failing test that describes the
   expected behaviour
2. **Green** -- write the minimal code to make the test
   pass
3. **Refactor** -- clean up while keeping tests green

Run `cargo xtask test` after each step to confirm the
cycle. Do not skip ahead to implementation without a
failing test first.

## Commits

**All commits must go through the `/commit` skill.**
Never use `git commit` directly. No "Co-Authored-By",
no emoji.

## Acceptance Criteria

Before completing any task, run `cargo xtask validate`,
which checks:

1. **Formatting**: `cargo fmt --all -- --check`
2. **No warnings**:
   `cargo clippy --all-targets -- -D warnings`
3. **All tests pass**: `cargo test`
4. **Coverage >= 90%**
5. **Code duplication <= 6%** (production code, tests
   excluded)

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
| `/todo` | Add a TODO item or implement the next pending one |
| `/simplify` | Review changed code for quality |
| `/architect` | Project overview and architecture guide |
| `/web-dev` | Axum, Svelte 5, Vite, Playwright patterns |
| `/template-improve` | Log feedback for the rustbase template |
| `/template-sync` | Sync upstream template changes |

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
