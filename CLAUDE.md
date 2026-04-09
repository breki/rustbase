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
cargo xtask validate          # fmt + clippy + tests + coverage
cargo xtask test [filter]     # tests only
cargo xtask clippy            # lint only
cargo xtask coverage          # coverage only (>=90%)
cargo xtask fmt               # format code
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

## Commits

Use `/commit`. No "Co-Authored-By", no emoji.

## Acceptance Criteria

Before completing any task, run `cargo xtask validate`,
which checks:

1. **Formatting**: `cargo fmt --all -- --check`
2. **No warnings**:
   `cargo clippy --all-targets -- -D warnings`
3. **All tests pass**: `cargo test`
4. **Coverage >= 90%**

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
| `/commit` | Commit with versioning, diary, and code review |
| `/todo` | Process the next pending TODO item |
| `/simplify` | Review changed code for quality |
