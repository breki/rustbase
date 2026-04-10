# Red Team Findings -- Resolved

Archive of fixed red team findings, newest first.
See [redteam-log.md](redteam-log.md) for open findings.

---

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
