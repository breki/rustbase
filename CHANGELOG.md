# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed (docs)

- `CLAUDE.md` gains three new sections: "Workspace
  lints and xtask overrides" (the local-override
  recipe for derived projects that need OS-API code in
  xtask), "Edition-2024 migration notes" (the four
  mechanical fixes), and "Version source of truth"
  (sentinel + CARGO_PKG_VERSION conventions).
- `.template-sync.toml` header expanded into a
  ~15-line comment block describing the file's role,
  the managing skill, and cross-references to
  `template-feedback.md` and `/template-improve`.
- `docs/developer/template-feedback.md` restructured
  into three lifecycle sections (Open divergences /
  Resolved / Suggestions to flow back). Existing
  entries reclassified; section header explains how
  template-repo semantics map to derived-project
  semantics.
- `.claude/commands/template-improve.md` updated to
  route new entries by section.

### Added

- Coverage failures now include uncovered-line ranges
  for each module below `MODULE_THRESHOLD`. Saves the
  round trip of running a separate detail command at
  the moment a coverage failure happens. Renders as
  `<file>: <pct>%` followed by an indented
  `uncovered: 84-93, 209-221` line.
- `coverage::CoverageFailure` (typed enum) and
  `coverage::FailingModule` (public struct) expose
  structured failure data alongside the rendered
  string, ready for future programmatic consumers
  (JSON output, sort-by-worst, CI annotations).

### Changed

- `coverage::THRESHOLD` renamed to
  `coverage::OVERALL_THRESHOLD` for naming symmetry
  with `MODULE_THRESHOLD`.
- llvm-cov segment parsing now goes through a typed
  `Segment` struct (custom `Deserialize` from a
  6-element tuple). Shape mismatches with older
  llvm-cov versions surface as a hard parse error
  instead of silently misclassifying gap regions.

### Added (earlier in this cycle)

- `cargo xtask clean-cache` empties
  `target/{debug,release}/incremental/` while keeping
  the directories themselves. Reports bytes freed per
  directory plus a total. Manual invocation only --
  never auto-wired. Symlinks and Windows directory
  junctions are unlinked rather than followed
  (prevents accidental deletion outside the
  workspace); per-entry failures are collected and
  reported at the end rather than aborting (the AV-
  and rust-analyzer-resilience use case the tool
  exists for).
- `helpers::dir_size` and `helpers::fmt_bytes` general
  utilities; `helpers::temp_scratch` shared test-only
  scratch-directory helper (PID + thread id + atomic
  counter for parallel-test isolation without a
  `tempfile` dependency).
- `[profile.release]` defaults `incremental = true,
  codegen-units = 256` for faster iteration on
  personal-use deployments (override for
  performance-critical targets)
- `frontend/package.json` aggregator scripts
  `npm run fix` (prettier + eslint --fix) and
  `npm run check:all` (check, lint, format:check,
  test, build)

### Changed

- `cargo xtask validate` Test step now runs only
  `cargo test -p xtask` and prints as
  `Test (xtask only)`. Coverage already executes every
  non-xtask test under llvm-cov instrumentation, so the
  previous `--workspace` test pass was duplicated work.
  Net effect: validate is faster with no loss of signal.
- Validate's Test step now emits the same rich
  diagnostics (failing test names + assertion details)
  as `cargo xtask test` when failures occur, via a
  shared `report_failure` helper.

## [0.5.0] - 2026-05-17

### Added

- `cargo xtask deploy` / `cargo xtask deploy-setup` for
  one-shot and one-time deployment to a remote Linux
  host running systemd, with input-validated `.deploy`
  config and ssh/scp wrappers that avoid shell parsing
- Sandboxed `deploy/rustbase-web.service` systemd unit
  (binds `127.0.0.1` by default; `ProtectSystem=strict`,
  `MemoryHigh=256M` + `MemoryMax=1G`, full syscall
  filter)
- `.deploy.sample` config template and `.deploy`
  gitignored
- `docs/deployment.md` deployment guide
- `frontend/src/lib/sort.ts` with `Intl.Collator`-backed
  `compareNames` / `compareIds` helpers (case-insensitive;
  `compareIds` is numeric-aware; optional locale arg)
- `build.ps1 deploy` and `build.ps1 deploy-setup`
  wrappers
- `xtask/src/helpers.rs` gained a `workspace_root()`
  helper

## [0.4.0] - 2026-04-16

### Added

- `build.ps1 dev` command launches backend + frontend
  dev servers with one invocation (parses `.ports`,
  pre-builds the backend, kills descendants cleanly on
  Ctrl+C)
- Frontend TypeScript support: `tsconfig.json`,
  `typescript`, `@tsconfig/svelte`, `svelte-check`
  dev dependencies; `.ts` entry point; `lang="ts"` in
  `App.svelte` with typed API response interfaces and
  runtime `res.ok` narrowing
- `npm run check` script in `frontend/package.json`
- `cargo xtask validate` now runs `svelte-check` as
  step 6 (skipped gracefully when no frontend is
  present or `node_modules` is missing)
- Modular xtask with agent-friendly stepwise output
  (`[1/5] Fmt... OK (0.3s)`) and structured result types
- `cargo xtask check` fast compilation check
- `/check`, `/test`, `/validate` slash commands
- `/todo` dual-mode: add items with arguments, implement
  without
- Cross-platform `scripts/kill-servers.sh` and
  `scripts/e2e.sh` for E2E test workflow
- `docs/ai-agents/guidelines.md` for agent-consumed
  tooling conventions
- E2E test policy in `CLAUDE.md` (UI features require
  Playwright tests)
- Root `tsconfig.json` for TypeScript E2E tests and
  Playwright config

### Changed

- `/commit` skill: code reviews before E2E tests,
  expanded review scope (frontend, config, deployment
  files), Deployment category in Red Team prompt,
  all findings reported via `AskUserQuestion`
- Playwright config: `127.0.0.1` to `localhost`,
  `cd frontend` to `cwd` option, `.js` to `.ts`
- E2E smoke test renamed from `.spec.js` to `.spec.ts`

### Fixed

- `@eslint/js` pin corrected from `^10.2.0` to
  `^10.0.0` (10.2.0 was never published to npm, so
  `npm install` failed with `ETARGET` on clean clones)
- Vite dev proxy now forwards `/health` to the backend
  (previously only `/api/*` was proxied, which broke
  the `health endpoint returns OK` E2E test against
  the frontend origin)
- `vitest` config: `passWithNoTests: true` prevents
  failure with no test files
- xtask: `CARGO_TERM_COLOR=never` for all JSON-parsed
  cargo output (coverage, metadata)
- xtask: clippy noise lines (`generated N warning`)
  filtered from output
- `kill-servers.sh`: `pkill -x` instead of `pkill -f`;
  PowerShell `Get-CimInstance` instead of deprecated
  `wmic`

### Added

- Initial project template with workspace structure
- xtask build automation (validate, test, clippy, fmt,
  coverage)
- Claude Code configuration with Stop hook, commit
  skill with Red Team + Artisan code review
- GitHub Actions CI (multi-platform) and release
  workflow (5 targets)
- Development diary and code review finding logs
- Optional web app: Axum backend + Svelte 5/Vite
  frontend with dev proxy, SPA routing, health/status
  API endpoints
- PowerShell build script (`build.ps1`)
- Integration test scaffold with `assert_cmd`
- Playwright E2E test scaffold with auto-server start
- `.ports` config pattern for port management
- `.mise.toml` for Node.js version management
- `llms.txt` AI-agent reference (llmstxt.org)
- `/architect` and `/web-dev` Claude Code skills
- CI frontend build job; release packages both
  binaries with frontend dist
- Code duplication check (`cargo xtask dupes`) using
  `code-dupes` with 6% threshold
- `/template-improve` slash command for logging
  template feedback
- TDD (red/green/refactor) guidance in `CLAUDE.md`
- Frontend linting with ESLint + `eslint-plugin-svelte`
- Frontend formatting with Prettier +
  `prettier-plugin-svelte`
- Frontend unit testing with Vitest +
  `@testing-library/svelte`
- `/template-sync` slash command for syncing upstream
  template changes into derived projects
- `.template-sync.toml` for tracking template version
  origin and last sync point

### Fixed

- `/health` endpoint now returns JSON (`{"status":"ok"}`)
  instead of plain text for API consistency
- `vite.config.js` uses `import.meta.dirname` instead
  of CommonJS `__dirname`
- Tokio dependency narrowed from `full` to explicit
  feature list (`macros`, `rt-multi-thread`, `net`,
  `signal`)
- Release workflow uses `Compress-Archive` instead of
  `7z` for Windows packaging
- Release workflow warns when CHANGELOG extraction
  produces empty release notes
- Coverage no longer fails out of the box by excluding
  `xtask` crate and binary `main.rs` entry points
- Clarified `anyhow` vs `thiserror` dependency split
  in `Cargo.toml` comments
- Enforced that all commits must use `/commit` skill
- Release workflow uses `env:` blocks instead of inline
  `${{ }}` interpolation in `run:` blocks
- Release workflow fails on empty release notes instead
  of just warning
- Release checksum generation uses `nullglob` and
  explicit archive globs
- Release notes extraction uses exact version match
  instead of substring
- `create_router` accepts `&Path` instead of `&str`
- CLI bind address parsed as `IpAddr` via clap instead
  of string format + parse
- Added `edition = "2024"` to `rustfmt.toml`
- Documented `code-dupes` prerequisite in README
