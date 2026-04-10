# Artisan Findings -- Resolved

Archive of fixed Artisan code quality findings, newest
first. See [artisan-log.md](artisan-log.md) for open
findings.

---

### AQ-004 -- Stringly-typed errors throughout xtask

- **Date:** 2026-04-10
- **Category:** Type Safety
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** Kept `Result<(), String>` but
  structured error messages with consistent prefixes
  ("failed to run", "exited with") so callers can
  pattern-match on content. Added conditional install
  hint in `run_dupes()` that checks the prefix.

### AQ-003 -- Install hint on all code-dupes errors

- **Date:** 2026-04-10
- **Category:** Error Handling
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** `run_dupes()` now only appends the
  install hint when the error contains "failed to run"
  (command not found), not when code-dupes exits
  non-zero due to excessive duplication.

### AQ-002 -- Loop-invariant threshold allocation

- **Date:** 2026-04-10
- **Category:** API Design
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** Hoisted `threshold` string above the
  loop. Used `:.1` format for consistency.

### AQ-001 -- Hardcoded crate paths vs workspace-aware

- **Date:** 2026-04-10
- **Category:** Abstraction Boundaries
- **Commit context:** v0.1.1 template feedback fixes
- **Resolution:** Replaced hardcoded paths with
  `discover_src_dirs()` which uses `cargo metadata` to
  dynamically discover workspace members, consistent
  with how `run_coverage()` uses `--workspace`.
