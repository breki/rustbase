# TODO

Project work queue.

- `/todo <text>` captures a new item with a generated slug.
- `/todo` (no arguments) lists pending slugs.
- `/implement <slug>` plans and implements a pending item.
- `/implement` (no arguments) lists pending items and asks
  which to act on.

Each implemented item gets a planning doc at
`docs/issues/<slug>.md` that captures the problem statement,
plan, decisions, and outcome.

## Pending

<!-- Items captured by /todo land here. -->

- **coverage-metadata-ignore** -- Make the coverage
  `IGNORE_REGEX` extensible via
  `[workspace.metadata.coverage] ignore = [...]` in the
  root `Cargo.toml`. The hardcoded default in
  `xtask/src/coverage.rs:23`
  (`r"src[/\\]main\.rs$"`) stays as the baseline; user
  patterns are merged in. Every derived project that
  excludes a hardware-bound submodule (the recipe added
  in CLAUDE.md's "Coverage exceptions for hardware-
  bound code") currently has to fork `coverage.rs` --
  metadata-based config keeps the xtask unmodified.
  Includes tests for merge behaviour and missing-key
  graceful fallback. Sourced from kozmotic's template
  feedback (2026-05-04).

- **xtask-strip-web** -- Ship `cargo xtask strip-web`
  as a one-shot in-place mutation that converts the
  template into a CLI-only project. Touches:
  `crates/rustbase-web/` (delete), `frontend/` (delete),
  `Cargo.toml` workspace members (remove web crate),
  `.github/workflows/` (remove frontend job),
  `build.ps1` (remove Invoke-Dev/Invoke-Frontend/
  Invoke-E2E functions and dispatch branches),
  `README.md` + `llms.txt` (drop web-app sections),
  `CLAUDE.md` (drop Frontend Development + E2E Testing
  sections), `scripts/e2e.sh` + `scripts/kill-servers.sh`
  (delete, orphaned without frontend), `.ports.sample`,
  and `playwright.config.ts` + root `tsconfig.json`.
  Requires a git-clean precondition check (irreversible
  in-place edit). Should land with its own xtask tests
  exercising the file-mutation logic against a fixture
  workspace. Significant scope -- worth a focused
  session and a `docs/issues/xtask-strip-web.md`
  planning doc up front. Sourced from kozmotic's
  template feedback (2026-05-04).

## Done

- [**changelog-version-drift**](issues/changelog-version-drift.md)
  -- Backfilled dated CHANGELOG sections 0.10.1-0.15.0 from git
  history; `[Unreleased]` now holds only genuinely-unreleased
  work. (2026-07-16)

<!-- Completed items are moved here by /implement during
     finalisation, linked to their issue doc. -->
