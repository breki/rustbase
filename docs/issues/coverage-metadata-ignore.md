# coverage-metadata-ignore

**Status:** Done
**Captured:** unknown (pre-2026-07-16; from kozmotic feedback)
**Started:** 2026-07-16
**Completed:** 2026-07-16

## Problem

Make the coverage `--ignore-filename-regex` extensible via
`[workspace.metadata.coverage] ignore = [...]` in the root
`Cargo.toml`, so a derived project that must exclude a
hardware-bound submodule (see CLAUDE.md "Coverage exceptions
for hardware-bound code") configures it in its manifest instead
of forking `xtask/src/coverage.rs`. The hardcoded default stays
as the baseline; user patterns are merged in. Include tests for
merge behaviour and missing-key graceful fallback.

## Context

- `xtask/src/coverage.rs:30` -- `const IGNORE_REGEX: &str =
  r"src[/\\](main\.rs$|bin[/\\])"` (the todo's cited value
  `src[/\\]main\.rs$` is stale; `bin/` was added since).
- `coverage.rs:92-100` -- `IGNORE_REGEX` is passed verbatim as
  the single `--ignore-filename-regex` argument to
  `cargo llvm-cov`.
- xtask has **no `toml` dependency** (only clap, serde,
  serde_json, windows-sys). The codebase deliberately
  hand-parses manifests rather than pulling a TOML crate
  (`gate.rs` parses `Cargo.lock`; `deploy_guard.rs` parses
  `Cargo.toml` `[package]`/`[workspace.package]`). This change
  follows that convention -- no new dependency.
- Root `Cargo.toml` currently has no `[workspace.metadata]`
  section, so "absent -> empty, use default only" is the common
  path (the template itself adds no ignores).
- `helpers::workspace_root()` gives the repo root for reading
  the manifest.
- CLAUDE.md "Coverage exceptions for hardware-bound code" step
  2 currently tells derived projects to "add the leaf
  submodule to the coverage `IGNORE_REGEX` in
  `xtask/src/coverage.rs`" -- i.e. fork the xtask. This feature
  exists to replace that with manifest config; the doc must be
  updated to match.

## Open questions

- None blocking. Hand-parse vs `toml` crate resolved by canon
  (hand-parse). Regex combination is a plain `|` alternation.

## Plan

1. Add two pure helpers to `coverage.rs`:
   - `parse_coverage_ignore(manifest: &str) -> Vec<String>` --
     find `[workspace.metadata.coverage]`, read its `ignore`
     array (inline `["a","b"]` or multi-line), return the
     quoted patterns; empty when the section or key is absent.
     Tolerates comments, trailing commas, and whitespace.
   - `build_ignore_regex(default: &str, user: &[String]) ->
     String` -- return `default` unchanged when `user` is
     empty; else join `default` and each user pattern with `|`
     (top-level alternation; `user` order preserved).
2. In `coverage_check`, read the root `Cargo.toml` via
   `workspace_root()`, compute the merged regex, and pass it to
   `cargo llvm-cov --ignore-filename-regex` instead of the bare
   `IGNORE_REGEX` const. A missing/unreadable manifest degrades
   to the default (never fails the gate over config parsing).
3. Update CLAUDE.md "Coverage exceptions for hardware-bound
   code" step 2 to prescribe adding the path to
   `[workspace.metadata.coverage] ignore` in the root
   `Cargo.toml` (no xtask fork), and add a short
   `## Supply-chain hygiene`-style note near the config.
4. Add a commented example `[workspace.metadata.coverage]`
   block to the root `Cargo.toml` so the knob is discoverable.

## Test strategy

Rust unit tests in `coverage.rs` (pure, no llvm-cov needed):
- `parse_coverage_ignore`: inline array; multi-line array;
  absent `[workspace.metadata.coverage]` section -> empty;
  section present but no `ignore` key -> empty; comment /
  trailing-comma tolerance.
- `build_ignore_regex`: empty user -> default unchanged;
  one/many user patterns -> correct `|`-joined alternation with
  order preserved.
The wiring in `coverage_check` (file read + llvm-cov spawn) is
I/O, exercised implicitly when `validate` runs coverage; xtask
is excluded from the coverage gate itself (`--exclude xtask`),
so no coverage-floor concern.

## Decisions

- 2026-07-16: **Hand-parse the manifest, no `toml` crate.**
  Matches the existing convention (`gate.rs`,
  `deploy_guard.rs`) and avoids a new dependency (which would
  also carry a cooldown). The `ignore` array is a small,
  regular shape a focused parser handles.
- 2026-07-16: **Merge by `|` alternation**, default first, user
  patterns appended in declared order. A missing/unreadable
  manifest falls back to the default rather than failing.

## Progress log

- 2026-07-16: Added `parse_coverage_ignore`,
  `build_ignore_regex`, and helpers (`toml_section_name`,
  `strip_toml_comment`, `quoted_strings`) plus `ignore_regex()`
  to `coverage.rs`; wired `coverage_check` to the merged regex.
  6 unit tests. Updated CLAUDE.md recipe + root `Cargo.toml`
  example. clippy `if_not_else` fixed by leading with the
  positive branch. Verified end-to-end: with `ignore =
  ["rustbase-web"]` temporarily set, `cargo xtask coverage`
  line total dropped 94 -> 6 (the pattern reached llvm-cov);
  reverted. Absent-section path confirmed by a clean full
  `validate` (100%, unchanged from baseline).

- 2026-07-16 (review): the `/commit` reviewers found the naive
  line-scan parser was fragile for regex values. Rewrote
  `parse_coverage_ignore` around a quote-aware char scanner
  (`coverage_section_body` + `ignore_value_region` +
  `scan_quoted`): `]` and `#` inside a quoted pattern are now
  literal (RT-2/AQ-2 and RT-3), and both `"..."` and `'...'`
  (literal) strings are read. Added `validate_ignore_patterns`
  to drop blank entries and reject match-all patterns
  (`.`/`.*`/`.+`) with a loud error -- an empty or match-all
  pattern would otherwise silently neuter the gate (RT-1);
  `ignore_regex` now returns `Result`. Documented the
  supported-form limitation (no dotted-key / inline-table
  spellings; use single-quoted literals) in CLAUDE.md and the
  `Cargo.toml` example (RT-4). Deferred the shared-TOML-scan
  extraction (AQ-1/AQ-4) to `aq-2026-07-16-shared-toml-scan-
  helper`. Verified: `ignore = ["."]` now aborts
  `cargo xtask coverage` loudly; full `validate` green.

## Outcome

`cargo xtask coverage` now merges
`[workspace.metadata.coverage] ignore = [...]` from the root
`Cargo.toml` into its `--ignore-filename-regex` baseline
(`xtask/src/coverage.rs`: `ignore_regex`,
`build_ignore_regex`, `parse_coverage_ignore`). Derived
projects exclude a hardware-bound leaf module by adding its
path to the manifest -- no `xtask` fork. Baseline unchanged
when the section is absent; a missing/unreadable manifest
degrades to the baseline rather than failing the gate. Docs:
CLAUDE.md "Coverage exceptions for hardware-bound code" step 2
now prescribes the manifest knob, and the root `Cargo.toml`
carries a commented example. No new dependency (hand-parsed,
per the `gate.rs` / `deploy_guard.rs` convention).
