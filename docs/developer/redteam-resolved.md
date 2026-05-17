# Red Team Findings -- Resolved

Archive of fixed red team findings, newest first.
See [redteam-log.md](redteam-log.md) for open findings.

---

### RT-037 -- `[profile.release]` defaults risky for a template (cross-confirmed with AQ-031)

- **Date:** 2026-05-17
- **Category:** Project Configuration (Medium)
- **Commit context:** v0.6.0 ledgerstone improvements Batch A
- **Resolution:** Kept the `incremental = true,
  codegen-units = 256` defaults but expanded the comment
  block to document the non-default nature, the specific
  workloads that pay the optimisation cost (hot loops,
  allocator-heavy, monomorphisation-rich code), and
  explicit override guidance for CPU-bound services,
  multi-user binaries, and `cargo install` consumers.
  Replaced the unsubstantiated "<5%" claim with an
  attribution to Ledgerstone's single data point.

### RT-036 -- `test_check_xtask` regressed failure diagnostics (cross-confirmed with AQ-030)

- **Date:** 2026-05-17
- **Category:** Correctness (Medium)
- **Commit context:** v0.6.0 ledgerstone improvements Batch A
- **Resolution:** Extracted `report_failure(stdout, stderr)`
  from `test()` and reused it from both `test()` and
  `test_check_xtask()`. Validate's Test step now prints
  failing test names + assertion details to stderr the
  same way `cargo xtask test` does.

### RT-035 -- "Test OK" step output misleading after scope narrow

- **Date:** 2026-05-17
- **Category:** Correctness (Low)
- **Commit context:** v0.6.0 ledgerstone improvements Batch A
- **Resolution:** Renamed validate step label from `Test`
  to `Test (xtask only)` so the success line cannot be
  read as covering the whole workspace.

### RT-034 -- `parse_port` did not handle `--port=N` equals form

- **Date:** 2026-05-17
- **Category:** Correctness (Low / cosmetic)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** `parse_port` now skips an optional `=`
  after the `--port` token; also tightened to return
  `Option<u16>` with range validation via `.parse()`.

### RT-033 -- service unit `--bind 0.0.0.0` exposed by default

- **Date:** 2026-05-17
- **Category:** Deployment (Medium)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Default `--bind 127.0.0.1` in the systemd
  unit; updated the security-note header to point users at
  the reverse proxy / `IPAddressAllow=` path.

### RT-032 -- `MemoryMax=256M` hard cap would OOM-kill under modest load

- **Date:** 2026-05-17
- **Category:** Deployment (Medium)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Replaced single hard cap with soft
  `MemoryHigh=256M` + hard `MemoryMax=1G`; aligns with
  cgroups v2 best practice.

### RT-031 -- failed install left service stopped indefinitely

- **Date:** 2026-05-17
- **Category:** Correctness / Availability (High)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Pre-staged frontend before the stop
  window; on any failure during stop/swap/start the
  deploy now attempts `systemctl start` as a rollback
  before propagating the error.

### RT-030 -- "atomic swap" comment was inaccurate

- **Date:** 2026-05-17
- **Category:** Correctness (Low)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Replaced `rm -rf`-then-`mv` with a real
  POSIX-atomic swap (`mv current → old`, `mv new →
  current`, then `rm -rf old`). The path is never absent.

### RT-029 -- stale `frontend-dist-new` could nest files wrong

- **Date:** 2026-05-17
- **Category:** Correctness (Medium)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** INSTALL_FRONTEND now `rm -rf`'s any
  stale `frontend-dist-new`/`frontend-dist-old` before
  copying. Added a unit test asserting the cleanup line
  is present.

### RT-028 -- `poll_active_status` returned a garbled status string

- **Date:** 2026-05-17
- **Category:** Correctness (Low)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Switched the remote command to
  `... || true` and took only the first stdout line,
  so the returned token is always a clean systemd state
  word.

### RT-027 -- CHANGELOG not updated for 0.4.0

- **Date:** 2026-04-16
- **Category:** Project Configuration (High)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** Moved `[Unreleased]` content to
  `[0.4.0] - 2026-04-16` with Added / Fixed entries for
  this release.

### RT-026 -- `npm run check` not wired into any pipeline

- **Date:** 2026-04-16
- **Category:** Correctness (Medium)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** Added `xtask/src/frontend_check.rs`
  and wired `svelte-check` into `cargo xtask validate`
  as step 6. Skips gracefully when frontend or
  `node_modules` is absent.

### RT-025 -- `allowImportingTsExtensions` unused in tsconfig

- **Date:** 2026-04-16
- **Category:** Project Configuration (Low)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** Removed. Also dropped other redeclared
  defaults already set by `@tsconfig/svelte` base.

### RT-024 -- `typescript: ^5.8.0` pin narrower than peers need

- **Date:** 2026-04-16
- **Category:** Project Configuration (Low)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** Relaxed to `^5.0.0` to match
  `@tsconfig/svelte` / `svelte-check` peer requirements.

### RT-023 -- Invoke-Dev has no backend pre-build

- **Date:** 2026-04-16
- **Category:** Correctness (Medium)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** `Invoke-Dev` now runs `cargo build -p
  rustbase-web` up front and launches the compiled
  binary directly. Compile errors surface immediately
  instead of silently behind a running frontend.

### RT-022 -- `.ports` config silently ignored by Invoke-Dev

- **Date:** 2026-04-16
- **Category:** Correctness (High)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** Added `Get-BackendPort` helper that
  parses `.ports` the same way `vite.config.js` does,
  and passes `--port $backendPort` to the backend.
  Parallel worktrees now work with `Invoke-Dev`.

### RT-021 -- Invoke-Dev orphaned the backend on Ctrl+C

- **Date:** 2026-04-16
- **Category:** Security / Correctness (High)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** Combined two fixes: (1) launch the
  already-built `rustbase-web.exe` directly instead of
  via `cargo run`, so there is no cargo shim process;
  (2) in the `finally` block, enumerate descendants via
  `Get-CimInstance Win32_Process -Filter
  "ParentProcessId=..."` and stop them before the
  parent, as belt-and-suspenders.

### RT-020 -- No port range validation in Playwright config

- **Date:** 2026-04-15
- **Category:** Correctness (Low)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Low severity, not fixed -- `.ports`
  file is developer-controlled local config.

### RT-019 -- helpers.rs tests don't call step_output

- **Date:** 2026-04-15
- **Category:** Correctness (High)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Extracted `format_step()` returning
  `String`, tests now call the actual function.
  `step_output()` delegates to `format_step()`.

### RT-018 -- Clippy "generated N warning" noise lines

- **Date:** 2026-04-15
- **Category:** Correctness (Low)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Added `.contains("generated")` filter
  to `extract_warning_lines()`.

### RT-017 -- cargo metadata missing CARGO_TERM_COLOR

- **Date:** 2026-04-15
- **Category:** Correctness (Medium)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Added `.env("CARGO_TERM_COLOR", "never")`
  to `discover_src_dirs()` cargo metadata command.

### RT-016 -- coverage.rs missing CARGO_TERM_COLOR

- **Date:** 2026-04-15
- **Category:** Correctness (High)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Replaced raw `Command::new` with
  `run_cargo_capture()` which sets the env var.

### RT-015 -- wmic deprecated on Windows 11

- **Date:** 2026-04-15
- **Category:** Correctness (Medium)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Replaced `wmic` with PowerShell
  `Get-CimInstance` + `Stop-Process` in
  `kill-servers.sh`.

### RT-014 -- pkill -f too broad in kill-servers.sh

- **Date:** 2026-04-15
- **Category:** Security (Medium)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Changed `pkill -f rustbase-web` to
  `pkill -x rustbase-web` for exact process name match.

### RT-013 -- awk version extraction substring match

- **Date:** 2026-04-10
- **Category:** CI/CD (Low)
- **Commit context:** v0.2.1 review finding fixes
- **Resolution:** Changed `index($0, "[" ver "]")` to
  `$0 ~ "^## \\[" ver "\\]"` for exact pattern match.

### RT-012 -- sha256sum glob fails without nullglob

- **Date:** 2026-04-10
- **Category:** CI/CD (Medium)
- **Commit context:** v0.2.1 review finding fixes
- **Resolution:** Added `shopt -s nullglob` with array
  collection and empty-check before `sha256sum`.

### RT-011 -- `sha256sum *` glob fragile in release

- **Date:** 2026-04-10
- **Category:** CI/CD (Low)
- **Commit context:** v0.2.1 review finding fixes
- **Resolution:** Replaced `sha256sum *` with explicit
  `*.tar.gz *.zip` globs via nullglob array.

### RT-010 -- Empty release notes don't block release

- **Date:** 2026-04-10
- **Category:** CI/CD (Medium)
- **Commit context:** v0.2.1 review finding fixes
- **Resolution:** Changed `::warning` to `::error` +
  `exit 1` so release fails if CHANGELOG extraction
  produces empty notes.

### RT-009 -- Inline `${{ }}` in release run blocks

- **Date:** 2026-04-10
- **Category:** CI/CD (Medium)
- **Commit context:** v0.2.1 review finding fixes
- **Resolution:** Moved all `${{ }}` expressions to
  `env:` blocks and referenced via `$STAGING`,
  `$TARGET`, `$env:STAGING`, `$env:TARGET`.

### RT-008 -- Frontend test leaks fetch mock

- **Date:** 2026-04-10
- **Category:** Correctness (Low)
- **Commit context:** v0.1.2 template feedback fixes
- **Resolution:** Used `vi.stubGlobal` with
  `afterEach(vi.restoreAllMocks)` to properly clean up
  the global fetch mock between tests.

### RT-007 -- Zip archive missing parent directory

- **Date:** 2026-04-10
- **Category:** Correctness (Medium)
- **Commit context:** v0.1.2 template feedback fixes
- **Resolution:** Changed `Compress-Archive -Path
  "$staging/*"` to `Compress-Archive -Path $staging`
  so the zip contains a top-level directory, matching
  the tar.gz archive structure.

### RT-006 -- Threshold formatting inconsistent

- **Date:** 2026-04-10
- **Category:** Correctness (Low)
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** Used `:.1` format specifier for all
  threshold displays for consistency with coverage.

### RT-005 -- Hardcoded crate list drifts from workspace

- **Date:** 2026-04-10
- **Category:** Correctness (Medium)
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** Replaced hardcoded paths with
  `discover_src_dirs()` using `cargo metadata` to
  dynamically find workspace member src directories.

### RT-004 -- run_dupes() silently succeeds with no dirs

- **Date:** 2026-04-10
- **Category:** Correctness (Medium)
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** `discover_src_dirs()` returns `Err`
  when no src directories found, failing the validate
  pipeline instead of silently passing.

### RT-003 -- run_dupes() breaks if cwd != project root

- **Date:** 2026-04-10
- **Category:** Correctness (Medium)
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** `discover_src_dirs()` uses absolute
  paths from `cargo metadata` manifest_path, so it
  works regardless of working directory.

### RT-002 -- Coverage regex misses rustbase-web main.rs

- **Date:** 2026-04-10
- **Category:** Correctness (High)
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** Simplified regex to `(^|[/\\])main\.rs$`
  which matches all `main.rs` files regardless of
  directory depth.

### RT-001 -- Coverage regex broken on Windows paths

- **Date:** 2026-04-10
- **Category:** Correctness (High)
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** Updated regex to match both forward
  and back slashes: `(^|[/\\])main\.rs$`.
