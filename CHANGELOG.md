# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `cargo xtask backfeed-diff` / `backfeed-record` accept a
  `--name <project>` override for the ledger key, for worktree
  -style downstream layouts (`../project/main`) whose path
  basename is a branch name rather than the project.

### Changed

- `cargo xtask dep-age-check` now runs as the **first**
  `validate` step (was last), so a dependency adopted within
  the 14-day cooldown fails the gate before the compile steps
  build and run its build script. It stays a no-op on unchanged
  lockfiles, and a connectivity failure still degrades to a
  warning; `audit` remains the last (network) step.

### Fixed

- `cargo xtask dep-age` now percent-encodes the package name in
  the registry URL, so a scoped npm name (`@scope/name`) resolves
  instead of 404-ing on the un-encoded `/`.
- The shipped systemd unit (`deploy/rustbase-web.service`) placed
  `StartLimitIntervalSec` / `StartLimitBurst` in `[Service]`,
  where systemd silently ignores them; moved to `[Unit]` so the
  crash-loop cap applies, and switched to
  `Wants=/After=network-online.target`.
- A downstream adopting only the sync/feedback half of the
  template tooling (and deleting the template-repo-only
  `backfeed` module) no longer hits dead-code build failures:
  the backfeed-only date helpers moved out of the shared
  `helpers` module into `backfeed`.

### Removed

- **BREAKING:** GitHub Actions CI/CD removed
  (`.github/workflows/ci.yml` and `release.yml`). Quality
  gating now runs locally via `cargo xtask validate`;
  releases are cut with `/release` + `cargo xtask deploy`.
  Derived projects no longer get CI checks or tag-triggered
  release builds out of the box.

## [0.16.0] - 2026-07-16

### Added

- Template-maintenance tooling moved into `cargo xtask`
  (deterministic, unit-tested) so `/template-backfeed`,
  `/template-improve`, and `/template-sync` scale without the
  LLM re-scanning growing markdown logs. New subcommands:
  `backfeed-diff <ds-path>` / `backfeed-record <ds-path>
  --watermark <date>` (a per-downstream watermark ledger in
  `docs/developer/backfeed-ledger.toml`, so backfeed evaluates
  only feedback newer than the last run); `feedback-add
  --section <open|resolved|suggestion> --title <t>` (appends a
  `template-feedback.md` entry with a minted `tf-<date>-<slug>`
  ID, inserted at the section top, deduped); and
  `sync-candidates <last-synced>` (the `/template-sync` file
  delta, categorized, with template-internal bookkeeping files
  filtered out).
- `cargo xtask coverage` merges
  `[workspace.metadata.coverage] ignore = [...]` from the root
  `Cargo.toml` into its `--ignore-filename-regex` baseline
  (which already excludes `src/main.rs` and `src/bin/`). Derived
  projects exclude a hardware-bound leaf module by adding its
  path to the manifest instead of forking `xtask`. The baseline
  is unchanged when the section is absent; a missing/unreadable
  manifest degrades to the baseline rather than failing the
  gate.
- `/release` command -- a SemVer release workflow that pairs
  with `/commit`. It infers the bump from the accumulated
  `[Unreleased]` CHANGELOG entries (`**BREAKING:**` or a
  non-empty `### Removed` -> major, `### Added` -> minor, else
  patch; override available), bumps
  `crates/rustbase/Cargo.toml`, promotes `[Unreleased]` to a
  dated section, runs `cargo xtask validate` as the release
  gate, commits the bookkeeping directly, and creates an
  annotated `vX.Y.Z` tag. `/release` is the sole version-bumper.
- `/html-report` command + `docs/ai-agents/html-report-template.html`
  -- produce a polished, self-contained local HTML page
  (analysis / report / reference) from the in-repo template and
  open it in the browser. All CSS/JS inlined, images as `data:`
  URIs, CSP-`<meta>`-enforced, dual light/dark theming, one
  `--fs-*` type scale; fetched content is HTML-escaped as data;
  never a cloud Artifact.

### Changed

- `/commit` is now a save-point: it no longer bumps the
  version, syncs `Cargo.lock`, or runs `cargo xtask validate`.
  Those move to the new `/release` command. Multiple commits
  land between releases; `/release` computes one bump from the
  accumulated `[Unreleased]` entries.
- `cargo xtask deploy` now refuses to run unless `HEAD` is on a
  `vX.Y.Z` annotated tag matching `crates/rustbase/Cargo.toml`
  and the working tree is clean -- tying "publish to
  production" to "cut a release" (`/release`). Run `/release`
  before deploying.

## [0.15.0] - 2026-07-15

### Added

- `cargo xtask dep-preflight` -- pre-compile cooldown
  remediation for the Rust dependency tree. After adding or
  bumping a dependency (via `cargo add`, which updates the
  lockfile without compiling), run this instead of going
  straight to `cargo build`: it inspects the newly-locked
  versions and pins any crate still within the 14-day cooldown
  down to its newest aged version (`cargo update --precise`),
  looping until the whole changed set is aged or no aged
  version satisfies the resolved requirements. Every step
  touches only the registry index and the lockfile -- no crate
  tarball is fetched and no build script runs until the tree
  is clean -- closing the window the post-resolution
  `dep-age-check` gate cannot (by the time that gate fails,
  cargo has already compiled the fresh crates). Front-door
  only: it cannot intercept a bare `cargo build`; that is what
  cargo's in-resolver `-Zmin-publish-age` (nightly) will
  eventually do automatically. Rust / crates.io only; requires
  `curl` + `git` + `cargo`.

## [0.14.0] - 2026-07-15

### Added

- `/update-deps` command -- an end-to-end third-party
  dependency upgrade workflow (Rust + frontend) that adopts
  the newest version of each dependency outside the 14-day
  cooldown, re-pins any `cargo update` picks that landed
  within the window, runs the frontend major-bump procedure,
  verifies with `validate`, and hands off to `/commit`.
- `cargo xtask dep-age ... --latest-aged` -- prints the
  highest version of a package that has cleared the cooldown
  (selected by version, skipping prereleases, yanked crates,
  and unpublished npm versions) -- the pin target
  `/update-deps` feeds to `cargo update --precise` /
  `npm install`.

## [0.13.0] - 2026-07-15

### Added

- `cargo xtask dep-age-check` -- enforces the 14-day cooldown
  as the final `validate` step, checking only the
  dependencies added or version-bumped in the working tree
  versus `HEAD` (both `Cargo.lock` and the frontend lockfile).
  Free (no network) when the lockfiles are unchanged; an
  unreachable registry or missing `HEAD` baseline degrades to
  a warning. A fresh version adopted with justification (or a
  security fix) is waved through by naming it in the
  `RUSTBASE_DEP_AGE_ALLOW` env var (`name@version`,
  comma-separated). Requires `curl` + `git`.

### Changed

- Refreshed third-party dependencies within the 14-day
  cooldown. Frontend major bumps: `typescript ^5 -> ^6`
  (6.0.3), `jscpd ^4 -> ^5` (5.0.11), `prettier-plugin-svelte
  ^3 -> ^4` (4.1.1); jscpd 5 drops ~107 transitive packages.
  Rust: 17 crates advanced to their newest out-of-cooldown
  version (`anyhow`, `hyper`, `regex`, `serde_json`, `syn`,
  ... ), which also clears the last `cargo audit` advisory
  warning (0 vuln, 0 warn). Held back as too fresh: TypeScript
  7.0.2 (7 days) and the svelte 5.56.5 patch (1 day).

## [0.12.0] - 2026-07-14

### Added

- `cargo xtask audit` -- a security-advisory gate that runs
  `cargo audit` (RUSTSEC) over `Cargo.lock` and `npm audit`
  over the frontend, failing on any vulnerability (advisory
  *warnings* like unsound/unmaintained are reported but do
  not fail). Wired into `validate` as the final step.
  Requires `cargo-audit` installed and network access to the
  advisory DB / npm registry.
- `cargo xtask dep-age <npm|cargo> <package> [version]` -- a
  dependency-cooldown helper that reports how many days ago a
  version was published and exits non-zero when it is younger
  than the 14-day cooldown (the window a malicious release is
  most likely still live). On-demand, a single package;
  requires `curl`.

## [0.11.0] - 2026-07-14

### Added

- `cargo xtask frontend-fmt [--check]` runs Prettier over
  the frontend (auto-fix by default, read-only under
  `--check`), wired into `validate` mirroring the Rust `fmt`
  gate.
- `cargo xtask frontend-dupes` gates frontend code
  duplication with jscpd (threshold 6%, tests excluded),
  wired into `validate` mirroring the Rust `dupes` gate.
  `.svelte` files are routed through jscpd's TypeScript
  tokenizer (via `frontend/.jscpd.json`), which it otherwise
  silently skips.
- `cargo xtask frontend-test` runs the frontend unit suite
  (vitest), wired into `validate` -- previously the
  component tests were ungated (svelte-check only
  type-checks), so a broken assertion passed `validate`.
- The starter app header now renders the build version
  (`__APP_VERSION__`, injected from `Cargo.toml`).

### Changed

- Upgraded the frontend toolchain to Vite 8 (Rolldown):
  `vite ^7 -> ^8`, `@sveltejs/vite-plugin-svelte ^6 -> ^7`
  (requires `svelte >= 5.46.4`, now the pinned floor),
  `vitest -> ^4.1.7`, and dropped the `{ hot: false }`
  vitest plugin option (no longer needed under v7). Builds
  run on the Rust-based Rolldown bundler (production build
  ~130ms). This also clears the frontend `npm audit`
  advisories (3 high, all in the old `vite 7.x`), so the
  dependency tree is advisory-clean.

## [0.10.4] - 2026-07-14

### Added

- `cargo xtask frontend-check` runs the frontend
  type check (svelte-check) standalone; skips cleanly when
  there is no frontend.
- `cargo xtask validate --check` checks formatting
  read-only (`fmt --check`) instead of auto-fixing it, for
  use in CI or before partial staging.

### Changed

- The Playwright E2E harness now runs on isolated ports
  separate from the dev server, so a run never collides with
  -- or silently reuses -- a dogfooding session.
  `scripts/e2e.sh` frees only its own e2e ports and no longer
  stops the dev servers; the e2e webServers run with
  `reuseExistingServer:false` and Vite uses `strictPort` on
  isolated runs. `playwright.config.ts` resolves the ports
  and pushes them to both webServers, so a bare
  `npx playwright test` self-isolates identically.
- `.ports` now configures **all four** ports --
  `backend_port` / `frontend_port` (dev) and
  `e2e_backend_port` / `e2e_frontend_port` (harness), each
  with a default; `E2E_BACKEND_PORT` / `E2E_FRONTEND_PORT`
  override the e2e keys. This lets multiple worktrees or
  rustbase-derived projects run concurrently by giving each
  a distinct four-port block. Shared port helpers moved to
  `scripts/lib/port-utils.sh`.
- `cargo xtask validate` runs its cheap static gates (Fmt,
  Duplication, Clippy, Frontend) before the expensive
  dynamic ones (Test, Coverage), so a fast check's failure
  is no longer gated behind the multi-minute instrumented
  Coverage run.
- `cargo xtask validate` now auto-fixes formatting in place
  by default; the previous read-only behaviour moved behind
  `--check`.
- `cargo xtask validate` prints `-> iterate with: cargo
  xtask <gate>` after a failed step so the single failing
  gate can be re-run in seconds rather than the whole
  pipeline.
- The frontend type-check gate now fails (instead of
  silently skipping) when `frontend/` exists but
  `frontend/node_modules` is not installed -- a skip that
  exits 0 is indistinguishable from a pass. A genuinely
  frontend-less (CLI-only) project still skips cleanly.

### Fixed

- `cargo xtask check` surfaces the `file:line` of
  compilation errors again. It runs with
  `--message-format=short` (diagnostics are path-prefixed
  single lines), but the extractor matched only
  `error[`-prefixed lines, so every located diagnostic was
  dropped and only the locationless summary survived.
- `cargo xtask clippy` now shows a denied lint's message
  and its `-->` source location. Under `-D warnings` a
  denied lint is reported as a bare `error:` (no `[Exxx]`
  code), which the extractor dropped -- printing an empty
  failure body -- and it also discarded the `-->` location
  line, so a failure named the lint but not where it fired.
- `cargo xtask test` compile-error output now includes the
  `-->` source-location line alongside each error message.

## [0.10.3] - 2026-07-13

### Fixed

- Frontend ESLint now parses TypeScript inside Svelte
  components. `eslint.config.js` had no TS-aware parser
  for `*.svelte`, so `npm run lint` failed with a parsing
  error on any `<script lang="ts">` using TS-only syntax
  -- including the starter `App.svelte`, which declares
  `interface`s, so the lint script was broken out of the
  box. Added `@typescript-eslint/parser` +
  `svelte-eslint-parser` and `files` blocks for `**/*.ts`
  and `**/*.svelte`; bumped `tsconfig` `lib` to ES2022 so
  modern APIs (e.g. `new Error(msg, { cause })`) type-check.

## [0.10.2] - 2026-07-13

### Changed

- `scripts/kill-servers.sh` now frees the dev-server ports
  (`:3000`, `:5173`) by stopping the process *listening* on
  them, instead of killing by process/image name. A by-name
  kill is machine-wide and would also terminate a production
  instance of the same binary running on another port.
- `cargo xtask coverage` now excludes every module under
  `src/bin/` from the per-module coverage floor, not just
  `src/main.rs`. Multi-file binary shells are exercised only
  by spawned-subprocess integration tests, which llvm-cov
  cannot fully credit; testable logic belongs in the library
  crate.

### Fixed

- `scripts/e2e.sh` now runs from the project root, so
  `npx playwright test` resolves `playwright.config.ts`
  regardless of the caller's working directory. A caller
  in a subdirectory previously got a silent "No tests
  found" zero-test pass instead of an error.
- `cargo xtask coverage` now runs `cargo llvm-cov clean`
  before measuring. Stale `.profraw` files from earlier
  runs were merged into the totals and inflated the line
  denominator, so a real 98% could read as ~70% --
  indistinguishable from a genuine coverage regression.
- `cargo xtask test <filter>` now fails when a filtered
  run matches zero tests instead of printing `Test OK`.
  `cargo test <filter>` exits 0 on no match, so a typo'd
  or over-specific filter previously read as a passing
  targeted run. Unfiltered runs are unaffected.

## [0.10.1] - 2026-05-18

### Changed

- `[profile.release]` reverted to cargo defaults so
  `cargo build --release` (and deploy flows) produce
  fully-optimised binaries. The previous
  `incremental = true` / `codegen-units = 256`
  overrides moved into a new
  `[profile.release-fast]`. Local fast iteration
  uses `cargo build --profile release-fast`;
  deployed binaries get cargo's standard
  `release` shape. Existing derived projects that
  relied on the old overrides should switch their
  iteration scripts to `--profile release-fast`.

### Fixed

- `cargo xtask clean-cache` no longer follows Windows
  directory junctions (`mklink /J`) under
  `target/incremental/`. A junction previously fell
  through `FileType::is_symlink()` (which on Windows
  is only set for `IO_REPARSE_TAG_SYMLINK`, not
  junctions) and was removed via `remove_dir_all`,
  which traversed the junction and deleted the
  *target* tree outside the workspace. The same
  blind spot also affected size reporting -- `dir_size`
  walked through junctions and inflated the "freed"
  byte count with the target tree's contents. Both
  sites now share an `is_reparse_or_symlink_meta`
  helper that checks `FILE_ATTRIBUTE_REPARSE_POINT`.
- `.claude/hooks/stop-check.sh` now labels which
  stage failed and includes its full output. The
  previous `cargo fmt --check && cargo xtask clippy
  && cargo xtask test` short-circuit captured only
  the first failing stage's output with no label, so
  when Claude fixed one stage the next-stage failure
  would resurface on the following hook run looking
  "new" -- defeating the `stop_hook_active` guard
  and amplifying fix loops.

## [0.10.0] - 2026-05-18

### Added

- `/template-backfeed` slash command -- inverse of
  `/template-sync`. Pulls template-improvement entries
  from a downstream rustbase-derived project's
  `docs/developer/template-feedback.md` back into this
  template repo. Validates the downstream is
  rustbase-based via its `.template-sync.toml`
  (normalized the same way as `/template-sync`),
  categorizes entries by rustwerk's `[Fixed locally]` /
  `[Logged, not fixed locally]` / `[N/A for]` tag
  convention, cross-references this template's own
  Resolved section to skip already-applied entries,
  treats downstream feedback as untrusted input (no
  bulk-apply), and logs each applied entry as a
  Resolved item before handing off to `/commit`.
  Aborts unless run from the template repo.
- Workspace `clippy.toml` with a curated
  `doc-valid-idents` allowlist (extending clippy's
  defaults via `".."`) so common Rust-infra terms
  (`PowerShell`, `JSON`, `FFI`, `WebSocket`, `macOS`,
  `GitHub`, `Tokio`, `Axum`, ...) don't trigger
  `clippy::doc_markdown` in doc comments. Derived
  projects append domain-specific identifiers rather
  than redefining the list.

### Changed

- `cargo xtask check` now prints the tail of stderr
  when cargo exits non-zero with no rustc-style error
  lines (manifest parse failures, corrupted
  `Cargo.lock`, network errors). Previously surfaced
  as `FAILED: 0 compilation error(s)` with no body.
  Also tightened the `aborting` filter so user errors
  whose message contains "aborting" are no longer
  silently dropped. Sourced from rustwerk's template
  feedback.
- `/template-sync` slash command tightened: removed
  the `Bash(git checkout:*)` permission it never
  used, hard-coded the upstream URL
  (`https://github.com/breki/rustbase`) with optional
  `.git` / trailing-slash normalization, and removed
  the "all" bulk-apply option (per-file or
  per-category opt-in only) to keep upstream
  diff content gated by user review. Sourced from
  rustwerk's template feedback.
- Stop hook (`.claude/hooks/stop-check.sh`) now runs a
  fast-path subset -- `cargo fmt --check && cargo xtask
  clippy && cargo xtask test` -- instead of full
  `cargo xtask validate`. Coverage (~15s), duplication,
  and frontend-check are skipped in the hook. fmt-check
  is included (~0.2s) because `/commit` only runs full
  validate for version-bumping commits; chores would
  otherwise let fmt drift through to CI. Full validate
  still runs from `/commit` for `feat`/`fix`/`perf`
  commits so coverage and duplication signal is
  preserved at that gate. Sourced from kozmotic's
  template feedback.
- Workflow retrospective extracted into a standalone
  `/retrospect` skill so it can be invoked manually
  mid-session (not just after a commit). `/commit`
  step 12 now delegates to `/retrospect` rather than
  inlining the rules; the recursive-skip carve-out
  for workflow-only diffs lives in the skill and
  applies only to auto-invocations, not when the
  user runs `/retrospect` directly.
- `/commit` step 5 now scans reviewer replies for
  truncated finding bodies (IDs referenced in a
  summary but with no labeled-bullet body present)
  and uses `SendMessage` to re-fetch them before
  presenting to the user. Closes a real drop observed
  in the prior session where two red-team findings
  were silently lost. Sourced from rustwerk's
  template feedback.
- `CLAUDE.md` gains a "Coverage exceptions for
  hardware-bound code" section documenting the
  extract-to-submodule + ignore-regex + `*_TEST_*`
  env-hatch recipe for I/O paths that can't run under
  llvm-cov in CI. Sourced from kozmotic's template
  feedback (the gate previously assumed everything was
  testable; real CLI projects routinely aren't).

### Removed

- `scripts/build.sh`, `scripts/clippy.sh`,
  `scripts/test.sh`, `scripts/fmt.sh`,
  `scripts/validate.sh` -- five one-line `cargo ...`
  wrappers redundant with `cargo xtask`. CLAUDE.md
  already steers users to xtask. `scripts/e2e.sh` and
  `scripts/kill-servers.sh` remain (non-trivial
  process-cleanup logic on Windows + Unix). Sourced
  from kozmotic's template feedback.

## [0.9.0] - 2026-05-17

### Added

- New `/implement` skill plans + executes a captured
  TODO item. Phase 1 writes `docs/issues/<slug>.md`
  (Problem, Context, Open questions, Plan, Test
  strategy, Decisions); Phase 2 codes with TDD per
  the refined CLAUDE.md rule; Phase 3 finalises
  (validate, status update, optional pre-launched
  reviewers in the background, manual verification,
  `/commit`).
- `docs/issues/` directory for per-item planning
  docs. Each implemented item gets a stable
  audit-trail document linked from `docs/todo.md`.

### Changed

- `/todo` skill split into capture-only behaviour:
  with arguments it captures a new item with a
  generated slug; without arguments it lists pending
  slugs. The "implement next pending item" mode
  moved to the new `/implement` skill, where
  planning and implementation are explicit phases.
- TODO list relocated from root `TODO.md` to
  `docs/todo.md`. The slug-prefixed bullet shape
  (`- **<slug>** -- summary`) is now part of the
  captured format so `/implement <slug>` can locate
  items unambiguously.
- `/template-sync` step 5 now cross-references
  `template-feedback.md`'s **Open divergences**
  section. Incoming template changes that conflict
  with a documented divergence are auto-flagged as
  **skip** with the divergence title inlined as the
  reason, instead of being re-proposed every sync.
- `/commit` step 5 now identifies **cross-confirmed**
  findings (same root cause flagged by both Red Team
  and Artisan -- whether same `file:line` or same
  defect in different vocabulary) and surfaces them
  under a dedicated heading with a combined
  `RT-NNN/AQ-NNN` ID. Empirically the strongest fix-
  signal in the review output.
- `/commit` gains step 12 -- a post-commit
  workflow retrospective covering Efficiency /
  Quality / Speed. Findings tagged `[trivial]` or
  `[propose]`; trivial ones get an "apply now?"
  prompt at the end. Skipped automatically when the
  diff is entirely under `.claude/**` or `CLAUDE.md`
  (recursive-skip carve-out).
- `CLAUDE.md` TDD section refined to distinguish
  **behaviour change** (strict red/green applies) vs
  **structural addition** (test + impl together).
  Removes the "write `unimplemented!()` first" theatre
  for self-contained new modules where the unit is
  too small to meaningfully fail-then-pass.
- `CLAUDE.md` gains three new sections: "Workspace
  lints and xtask overrides" (the local-override
  recipe for derived projects that need OS-API code in
  xtask), "Edition-2024 migration notes" (the four
  mechanical fixes), and "Version source of truth"
  (sentinel + `CARGO_PKG_VERSION` conventions).
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

## [0.8.0] - 2026-05-17

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

## [0.7.0] - 2026-05-17

### Added

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

## [0.6.0] - 2026-05-17

### Added

- `[profile.release]` defaults `incremental = true,
  codegen-units = 256` for faster iteration on
  personal-use deployments (override for
  performance-critical targets).
- `frontend/package.json` aggregator scripts
  `npm run fix` (prettier + eslint --fix) and
  `npm run check:all` (check, lint, format:check,
  test, build).

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
