# Red Team Findings -- Resolved

Archive of fixed red team findings, newest first.
See [redteam-log.md](redteam-log.md) for open findings.

---

### RT-061 -- Audit gate conflated a network failure with a vulnerability finding

**Category:** Reliability / gate integrity

**Resolution:** 2026-07-14 -- `cargo xtask audit` is a hard
`validate` step that reaches the network; an offline machine
or fresh CI run made it fail indistinguishably from a real
advisory, training operators to bypass the gate. Extracted a
pure `classify_audit` that keeps a positive vulnerability
count fatal but degrades a runner error (missing tool /
unreachable network / unparseable output) to a non-fatal
**warning** in the `validate` context; the standalone
command still errors. Unit-tested the fatal-vs-warning
boundary.

### RT-060 -- Frontend unit tests (vitest) were not part of any validate gate

**Category:** Coverage gap / process

**Resolution:** 2026-07-14 -- svelte-check only type-checks;
nothing ran the component tests, so RT-059 passed "all
gates." Added `cargo xtask frontend-test` (vitest via the
shared frontend runner) and wired it into `validate` as
step 7, so a broken frontend test now fails the gate.

### RT-059 -- App header change broke the frontend unit test, uncaught by validate

**Category:** Correctness / test regression

**Resolution:** 2026-07-14 -- Rendering `__APP_VERSION__` in
the `App.svelte` h1 made `npm test` throw (`__APP_VERSION__
is not defined` under vitest, which doesn't load the app
vite `define`) and broke the exact-text h1 assertion. Fixed
by adding the `define` to `vitest.config.js` and relaxing
the assertion to `toContain("rustbase")`. Slipped through
because vitest was ungated (see RT-060).

### RT-058 -- E2E harness reused the dev servers; a bare `npx playwright test` fell back to the dev ports

**Category:** Correctness / test isolation

**Resolution:** 2026-07-14 -- Stage 3. The harness shared
the dev ports with `reuseExistingServer:true`, and the
first isolation attempt left `e2e.sh` exporting unvalidated
raw `E2E_*` while the configs validated + fell back to dev
ports -- so a malformed value or a bare `npx playwright
test` targeted the dev servers. Fixed by making
`playwright.config.ts` the source of truth (resolves +
pushes ports to both webServers, `reuseExistingServer:false`)
and validating in `e2e.sh`; a bare run now self-isolates.

### RT-057 -- `.ports` port-positivity parity between bash and JS/TS (cross-confirmed AQ-054)

**Category:** Correctness / layer-disagreement

**Resolution:** 2026-07-14 -- `e2e.sh` rejected `0` /
leading-zero ports (`^[1-9][0-9]*$`) but `portFromFile` in
the configs accepted them (`^\d+$`), so `e2e_backend_port=0`
failed fast on the `e2e.sh` path yet bound port 0 (120s
hang) on bare npx. Tightened `portFromFile` to `^[1-9]\d*$`
in both configs.

### RT-056 -- `.ports` whitespace parity between bash and JS/TS (cross-confirmed AQ-053)

**Category:** Correctness / layer-disagreement (Windows)

**Resolution:** 2026-07-14 -- Bash `read_port` stripped only
literal spaces while the JS/TS twins stripped all whitespace
(`\s`), so a CRLF/tab in the git-ignored `.ports` made the
shell layer silently use defaults while the configs honored
the file -- divergent ports across entry paths on Windows.
Bash now strips `[[:space:]]`.

### RT-055 -- `run_frontend_check` doc claimed a graceful skip on missing `node_modules`

**Category:** Correctness (stale doc)

**Resolution:** 2026-07-14 -- The Stage-2 frontend-gate
change made a missing `node_modules` an error, but the
`validate.rs` doc still said it "skips gracefully" on that
condition. Reworded to: skips only when there is no
frontend; errors when `frontend/` exists but
`node_modules` is not installed.

### RT-054 -- `run_test` doc named the wrong Coverage step after the reorder

**Category:** Correctness (stale doc)

**Resolution:** 2026-07-14 -- The Stage-2 validate reorder
moved Coverage from step 4 to step 6, but `run_test`'s doc
still said "Coverage (step 4)". Corrected to step 6.

### RT-053 -- Frontend gate hard-fails local `validate` when `node_modules` is absent

**Category:** Behavior regression / DX

**Resolution:** 2026-07-14 -- Kept as intended (a skip that
exits 0 is indistinguishable from a pass); CI is unaffected
(the `frontend` job installs first, the `test` job never
calls the gate). Added a CLAUDE.md note that a fresh
full-stack checkout must run `npm --prefix frontend
install` before `validate`; the error message already names
that command.

### RT-052 -- `cargo generate-lockfile` folded an unrelated transitive dependency refresh into a scoped fix commit

**Category:** Project Configuration

**Resolution:** 2026-07-13 -- The /commit
`cargo generate-lockfile` step refreshed the whole
dependency tree (anyhow, bytes, http, hyper, regex,
serde_json, syn, ...), unrelated to the six Stage-1
tooling fixes and unmentioned in CHANGELOG/DIARY --
hard to bisect and hiding a supply-chain surface
change. Fix restored `Cargo.lock` to HEAD and ran
`cargo update -p rustbase`, so the only lock change is
the `rustbase` 0.10.1 -> 0.10.2 version entry.

### RT-048 -- `dir_size` still traversed Windows directory junctions after the `delete_entry` fix

**Category:** Correctness

**Resolution:** 2026-05-18 -- The hoard backfeed
patched the junction blind spot in `delete_entry` but
not in `dir_size`, which is called one line earlier in
`clear_dir_contents`. On Windows, `dir_size` recursed
through any junction in `target/incremental/`, summed
the target tree's bytes (inflating the "freed" report
and walking arbitrary external trees), and hit
unrelated `read_dir` errors. Fix moved the reparse-
point check into a shared `is_reparse_or_symlink_meta`
helper in `helpers.rs`, called from `dir_size`, and
hardened the regression test with a `freed < 4096`
assertion that would catch a future regression of the
same shape.

### RT-049 -- Junction regression test used `Path::exists`, which follows reparse points

**Category:** Correctness

**Resolution:** 2026-05-18 -- `assert!(!junction.exists())`
in the new junction test returns false for both "link
gone" and "link present but target unreachable",
so the test would falsely pass on a regression that
left a broken junction entry. Replaced with
`assert!(fs::symlink_metadata(&junction).is_err())`
which inspects the link entry directly.

### RT-050 -- `cmd /c mklink` in junction test was BatBadBut-exposed via `TMP`

**Category:** Security

**Resolution:** 2026-05-18 -- `cmd.exe`'s argument
re-parser (CVE-2024-24576 / BatBadBut) treats `&`,
`|`, `<`, `>`, `^`, `"`, `%` as metacharacters
regardless of how Rust's `Command::args` escapes them.
The test built its `mklink` invocation from
`temp_dir()` paths, which are user-controllable via
`TMP`/`TEMP`. Added an upfront check that refuses to
invoke `mklink` if either argument path contains a
cmd metacharacter; the test skips silently in that
case.

### RT-051 -- `[profile.release-fast]` shape was unpinned

**Category:** Project Configuration

**Resolution:** 2026-05-18 -- The new profile only
overrode `incremental` and `codegen-units`, leaving
`lto`, `debug`, `strip` to inherit whatever cargo
defaults in `[profile.release]`. The <5% runtime-cost
figure in the surrounding doc-comment was measured at
specific upstream defaults, so silent shifts in
cargo's release defaults would invalidate the claim.
Pinned `lto = false`, `debug = false`, `strip =
"none"` explicitly and noted the pin's intent.

### RT-044/AQ-040 -- `[Unreleased]` was a 4-version accumulator; v0.10.0 cut would have inherited 0.6-0.9 work (cross-confirmed)

**Category:** CHANGELOG correctness / release hygiene

**Resolution:** 2026-05-18 -- Retroactively split the
accumulated block into proper `[0.10.0]`, `[0.9.0]`,
`[0.8.0]`, `[0.7.0]`, and `[0.6.0]` sections,
attributing each bullet to its actual release via
DIARY entries and the git log between version tags.
The release commit now correctly attributes only the
post-0.9.0 work (rustwerk port, /template-backfeed,
clippy.toml, Stop-hook fmt-check, /retrospect
extraction, hardware-bound coverage docs, 5
script-wrapper removals) to v0.10.0.

### RT-045 -- Cargo.lock release-commit included an unrelated `tower-http` transitive bump

**Category:** Release hygiene

**Resolution:** 2026-05-18 -- `cargo generate-lockfile`
picked up a `tower-http 0.6.10 -> 0.6.11` registry
refresh unrelated to the version bump. Reverted the
lockfile to upstream HEAD and patched only the
rustbase package version line so the lockfile diff is
exactly one line.

### RT-046/AQ-041 -- DIARY entry title duplicated the version (cross-confirmed)

**Category:** DIARY formatting

**Resolution:** 2026-05-18 -- `Release v0.10.0 (v0.10.0)`
breaks the established `<description> (vX.Y.Z)`
convention. Rewrote the heading to describe the
release contents instead of restating the version
number.

### RT-043 -- CLI-only deletion list lacked ordering, leaving workspace transiently broken

**Category:** Correctness

**Resolution:** 2026-05-17 -- Re-ordered the CLI-only
"safe to delete" guidance in `CLAUDE.md` into a
numbered five-step procedure (edit `Cargo.toml`
members first, then `xtask validate`, then
`build.ps1`, then delete the now-unreferenced files,
then re-run `cargo xtask validate`) so the workspace
is never in a state where a member is gone but still
listed.

### RT-042 -- `/template-sync` upstream-URL match was too literal

**Category:** Security / Operational

**Resolution:** 2026-05-17 -- `/template-sync` now
normalizes the `.template-sync.toml` `repo` value by
stripping an optional trailing `/` and/or `.git`
suffix before comparing to the hard-coded canonical
upstream. The abort message names the canonical form
so users of pre-existing downstreams know what to
edit. Avoids breaking projects whose TOMLs were
written with the cargo-conventional `.git` suffix.

### RT-041 -- llvm-cov segment `is_gap_region` index assumption swallowed shape mismatches

- **Date:** 2026-05-17
- **Category:** Correctness (Medium)
- **Commit context:** v0.8.0 ledgerstone improvements Batch B (coverage detail)
- **Resolution:** Replaced positional `seg.get(N)`
  access with a typed `Segment` struct deserialised
  from a 6-element JSON tuple via a custom
  `Deserialize` impl. Older `llvm-tools-preview`
  versions that emit 5-element segments now surface
  as a hard "shape mismatch" parse error from
  `parse_segments`, rather than silently treating
  intentionally-skipped (gap) regions as uncovered.
  Regression test covers both the accepted 6-element
  shape and the rejected 5-element legacy shape.

### RT-040 -- `windows(2)` segment scan dropped the trailing uncovered segment (cross-confirmed with AQ-038)

- **Date:** 2026-05-17
- **Category:** Correctness (Medium)
- **Commit context:** v0.8.0 ledgerstone improvements Batch B (coverage detail)
- **Resolution:** Added an explicit trailing-segment
  branch after the `windows(2)` loop in
  `uncovered_ranges`: if the final segment is itself
  an uncovered, non-gap region, it now contributes a
  `(line, line)` entry to the raw range list before
  `merge_ranges` consolidates. Tests cover the
  single-segment file, the two-segment trailing-
  uncovered case, and the original mid-stream case.

### RT-039 -- `clean-cache` mid-loop `?` abort defeated its Windows-AV use case

- **Date:** 2026-05-17
- **Category:** Correctness (Medium)
- **Commit context:** v0.7.0 ledgerstone improvements Batch B (clean-cache)
- **Resolution:** `clear_dir_contents` now returns
  `(bytes_freed, Vec<String> errors)`. Each entry's
  deletion failure is captured into the vec and the loop
  continues; both incremental directories are walked
  even if one entry in the first dir is locked.
  `clean_cache` prints the error list after the totals
  and returns `Err(format!("{N} deletion error(s)"))`
  so the user sees a clear count and the affected paths.

### RT-038 -- `clean-cache` could follow symlinks/junctions outside the workspace

- **Date:** 2026-05-17
- **Category:** Correctness / Security (Medium)
- **Commit context:** v0.7.0 ledgerstone improvements Batch B (clean-cache)
- **Resolution:** Switched the deletion path from
  `path.is_dir()` + `fs::remove_dir_all` (both follow
  symlinks) to using `entry.file_type()` from the
  `DirEntry` (no traversal) and dispatching: symlinks
  go through `remove_file` on Unix and `remove_dir`
  with `remove_file` fallback on Windows so directory
  junctions get unlinked rather than recursed into.
  Added a regression test that plants a symlink to an
  outside tree inside the scratch incremental dir,
  runs the cleaner, and asserts the symlink target's
  contents survive.

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
