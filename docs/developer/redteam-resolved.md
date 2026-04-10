# Red Team Findings -- Resolved

Archive of fixed red team findings, newest first.
See [redteam-log.md](redteam-log.md) for open findings.

---

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
