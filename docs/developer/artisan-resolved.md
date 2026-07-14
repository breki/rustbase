# Artisan Findings -- Resolved

Archive of fixed Artisan code quality findings, newest
first. See [artisan-log.md](artisan-log.md) for open
findings.

---

### AQ-062 -- validate docstring didn't account for the network-dependent audit step

**Category:** Consistency (docs)

**Resolution:** 2026-07-14 -- The `validate` docstring
justified "cheap static then expensive dynamic"; the new
network-I/O Audit step fit neither. Extended the docstring
to state Audit runs last and degrades connectivity failures
to warnings so local gates aren't blocked on the network.

### AQ-061 -- dep-age cooldown decision was inline and untested

**Category:** Test coverage

**Resolution:** 2026-07-14 -- The `age < COOLDOWN_DAYS`
decision lived in the I/O `dep_age` fn (untested; xtask is
also outside the coverage gate). Extracted a pure
`cooldown_verdict(age, msg)` and unit-tested both branches
plus the 13/14-day boundary.

### AQ-060 -- dep-age `ecosystem` was a stringly-typed arg with a dead match arm

**Category:** API design / dead code

**Resolution:** 2026-07-14 -- `ecosystem` was a bare `String`
matched in both `fetch_registry` and `dep_age`, the latter's
`other =>` arm unreachable (fetch_registry already
validated). Replaced with a `clap::ValueEnum` `Ecosystem`
enum, so clap rejects bad values at the CLI boundary (with
`--help` enumeration) and both matches became exhaustive
two-arm matches.

### AQ-059 -- Three frontend `*_cmd` entry points duplicated the report match

**Category:** Duplication / maintainability

**Resolution:** 2026-07-14 -- The identical
skipped/OK/error match in each `frontend_*_cmd` was
extracted to `frontend::report_cmd(r, label)`; all four
wrappers (check, fmt, dupes, test) now call it.

### AQ-058 -- CLAUDE.md Acceptance Criteria under-described the validate gates

**Category:** Consistency (docs)

**Resolution:** 2026-07-14 -- The numbered list still showed
6 gates ending at "Frontend type check"; `validate` now
runs 9 (three more frontend gates + vitest). Updated the
list and the cheapest-first grouping so the canon (which
propagates to derived projects) matches what runs.

### AQ-057 -- `run_test` doc named a hardcoded coverage step number that keeps drifting

**Category:** Correctness (stale doc)

**Resolution:** 2026-07-14 -- The comment said "Coverage
(step 6)" (already re-numbered once in Stage 2, from 4);
the frontend gates pushed coverage to 8/9. Dropped the
parenthetical number entirely so it can't drift on the next
reorder.

### AQ-056 -- README listed `playwright.config.js` in the CLI-delete list

**Category:** Consistency (docs)

**Resolution:** 2026-07-14 -- The file is
`playwright.config.ts`; `README.md`'s "delete these for a
pure CLI template" list still named the `.js` form.
Corrected.

### AQ-055 -- `resolve_port` fail-fast relied on set -e through a command substitution

**Category:** Shell fragility / clarity

**Resolution:** 2026-07-14 -- `resolve_port` used `exit 1`
while only ever called in `$(...)`, so the abort worked
solely via set -e propagating the subshell status -- brittle
to future edits. Switched to `return 1` with an explicit
`|| exit 1` at each call site.

### AQ-054 -- `portFromFile` accepted `0` / leading zeros (cross-confirmed RT-057)

**Category:** Correctness / consistency

**Resolution:** 2026-07-14 -- See RT-057. `portFromFile` now
requires `^[1-9]\d*$`, matching `resolve_port`, so both
entry paths agree a `.ports` value is valid.

### AQ-053 -- bash / JS-TS `.ports` parsers disagreed on whitespace (cross-confirmed RT-056)

**Category:** Correctness / consistency (Windows)

**Resolution:** 2026-07-14 -- See RT-056. Bash `read_port`
now strips the same whitespace class (`[[:space:]]`) as the
JS/TS `.replace(/\s/g,"")` twins, so CRLF/tab `.ports` lines
parse identically across layers.

### AQ-052 -- CLAUDE.md Acceptance Criteria still described fmt as read-only

**Category:** Consistency (docs vs behaviour)

**Resolution:** 2026-07-14 -- Stage 2 flipped the default
`validate` fmt to auto-fix (read-only behind `--check`),
but CLAUDE.md's Acceptance Criteria still listed
`cargo fmt --all -- --check` as step 1. Updated the bullet
to describe the auto-fix default and the `--check` mode,
and added the cheapest-first ordering + fresh-checkout
npm-install note.

### AQ-051 -- `-->`-pairing state machine duplicated across two extractors

**Category:** Consistency / DRY

**Resolution:** 2026-07-14 -- The identical keep-then-pair
loop in `clippy_cmd::extract_warning_lines` and
`test_cmd::extract_compile_error_lines` was extracted to a
shared `helpers::pair_with_locations<F>(stderr, keep)`;
both call sites now pass only their keep-predicate.
Directly unit-tested in `helpers`.

### AQ-050 -- `extract_compile_error_lines` test missed the drop/reset branch

**Category:** Test coverage

**Resolution:** 2026-07-14 -- The test fed only lines that
hit the keep or paired-`-->` branch, never the else/reset
arm nor the "warning's `-->` is dropped" negative case.
Expanded the sample with a warning + its `-->` and a
non-matching `= note` line, asserting both are excluded.

### AQ-049 -- ESLint `**/*.ts` block could shadow Svelte 5 `.svelte.ts` module files

**Category:** Correctness / latent footgun

**Resolution:** 2026-07-13 -- The new plain-TS
`{ files: ["**/*.ts"], parser: tsParser }` block matched
`*.svelte.ts` too, and being later in the flat-config array
would (last-match-wins) override the svelte parser +
`svelte/svelte` processor that `svelte.configs.recommended`
sets for those files -- breaking rune globals (`$state`,
`$derived`) with false `no-undef` errors once a `.svelte.ts`
module is added. Dormant (none exist today). Fixed by adding
`ignores: ["**/*.svelte.ts"]` to the block.

### AQ-048 -- `xtask test` zero-test guard bypassed on the `--verbose` path

**Category:** API Design / behavioural consistency

**Resolution:** 2026-07-13 -- The new filtered-zero-tests
guard in `test()` sat after the `if opts.verbose { return
run_cargo_stream(...) }` early return, so
`cargo xtask test --verbose <typo>` still exited 0 on a
zero-match filter while the non-verbose form errored.
Documented the exemption at the verbose branch:
verbose streams raw output live (the human sees
`running 0 tests` directly) and captures no stdout to
count, so the guard's false-green risk only applies to
the condensed capture path that prints a bare `Test OK`.

### AQ-042 -- `clean_cache` double-prefixed `dir_size` warning paths

**Category:** Error Handling & Messages

**Resolution:** 2026-05-18 -- `clear_dir_contents`
wrapped each warning from `dir_size` with
`format!("size {}: {w}", path.display())`, but the
warning string already contained its own (deeper)
failing path. Operators saw two paths per message and
the first one named a parent rather than the actual
culprit -- the exact failure mode the `dir_size`
refactor was meant to fix. Now pushed verbatim via
`DirSizeWarning::to_string`.

### AQ-043 -- `dir_size` warnings were stringly typed

**Category:** API Design

**Resolution:** 2026-05-18 -- Replaced
`Vec<String>` warnings channel with a new
`DirSizeWarning { path: PathBuf, message: String }`
struct that implements `Display`. Callers that just
want the legacy format use `to_string()`; future
callers can filter/transform on the structured
`path` field.

### AQ-044 -- `dir_size` had two error channels with no semantic distinction

**Category:** API Design

**Resolution:** 2026-05-18 -- The previous signature
`Result<(u64, Vec<String>), String>` bifurcated
top-level failure (hard `Err`) from per-entry
failures (warnings), but the only caller funneled
both into the same error vector. Collapsed to
`(u64, Vec<DirSizeWarning>)` -- root-level failures
are now first entries in the warnings vector with
`path: <root>` and a zero total, removing API
complexity that paid no behaviour.

### AQ-045 -- `FILE_ATTRIBUTE_REPARSE_POINT` was a hand-typed Win32 constant

**Category:** Type Safety

**Resolution:** 2026-05-18 -- Replaced the inline
`const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400`
with the canonical
`windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT`.
Added `windows-sys` as a `cfg(windows)`-only
dependency in `xtask/Cargo.toml`; it's compile-time
free on non-Windows platforms.

### AQ-046 -- `delete_entry` doc-comment claimed "no extra syscall" after the junction fix

**Category:** Error Handling & Messages

**Resolution:** 2026-05-18 -- The original comment
described the pre-fix behaviour. The junction guard
now does one `symlink_metadata` per Windows entry
that's not already flagged as a symlink. Doc-comment
updated to say so explicitly; Unix path still
short-circuits on `file_type.is_symlink()` and pays
no extra syscall.

### AQ-047 -- `temp_scratch` panic message duplicated the function-name prefix

**Category:** Error Handling & Messages

**Resolution:** 2026-05-18 -- `panic!("temp_scratch:
create_dir_all({}) failed: {e}", ...)` duplicated
the location info that `panic!` already attaches.
Simplified to `panic!("failed to create scratch dir
{}: {e}", dir.display())` -- reads as a sentence,
gives the path and cause, and lets the panic
location identify the function for free.

### AQ-040 -- See RT-044 (cross-confirmed)

**Category:** CHANGELOG structure

**Resolution:** 2026-05-18 -- Same root cause as RT-044.
The duplicate `### Changed` / `### Added` blocks within
the would-be `[0.10.0]` section disappeared once the
accumulator was split into per-version sections.

### AQ-041 -- See RT-046 (cross-confirmed)

**Category:** DIARY entry style

**Resolution:** 2026-05-18 -- Same root cause as RT-046.

### AQ-039 -- `stderr_tail` allocated twice on the cold error path

**Category:** Type Safety

**Resolution:** 2026-05-17 -- Replaced
`lines[start..].to_vec()` with `lines.drain(..start)`
+ return `lines`, dropping the redundant second
allocation while keeping the same `Vec<&str>` return
type. Cold path so cosmetic, but the original pattern
read as accidental rather than deliberate.

### AQ-038 -- `windows(2)` segment scan lost trailing uncovered segment (cross-confirmed with RT-040)

- **Date:** 2026-05-17
- **Category:** Correctness / Reporting (Medium)
- **Commit context:** v0.8.0 ledgerstone improvements Batch B (coverage detail)
- **Resolution:** See RT-040. Explicit trailing-segment
  branch closes the windowed-iteration gap.

### AQ-037 -- `CoverageResult.error: Option<String>` discarded structured failure data

- **Date:** 2026-05-17
- **Category:** API Design (Medium)
- **Commit context:** v0.8.0 ledgerstone improvements Batch B (coverage detail)
- **Resolution:** Introduced `pub enum CoverageFailure`
  with `Overall { pct, threshold }` and
  `Modules(Vec<FailingModule>)` variants; made
  `FailingModule` `pub`. `coverage_check` now returns
  the structured failure; presentation moves to
  `pub fn format_failure(&CoverageFailure) -> String`,
  shared by validate and the standalone `coverage`
  command. Future consumers (CI annotations,
  sort-by-worst tooling, JSON export) can introspect
  failures directly without re-parsing the message.

### AQ-036 -- `coverage::THRESHOLD` vs `MODULE_THRESHOLD` naming asymmetry

- **Date:** 2026-05-17
- **Category:** API Design (Low)
- **Commit context:** v0.8.0 ledgerstone improvements Batch B (coverage detail)
- **Resolution:** Renamed `THRESHOLD` to
  `OVERALL_THRESHOLD`. Touched the one external call
  site in `xtask/src/validate.rs` and the success
  message in `coverage::coverage()`.

### AQ-035 -- Magic numeric indices `seg.get(0/2/3/5)` in coverage segment parser

- **Date:** 2026-05-17
- **Category:** Type Safety (Medium)
- **Commit context:** v0.8.0 ledgerstone improvements Batch B (coverage detail)
- **Resolution:** Subsumed by RT-041 -- the typed
  `Segment` struct replaces positional indices with
  named fields (`line`, `col`, `count`, `has_count`,
  `is_region_entry`, `is_gap`), exposed via an
  `is_uncovered()` predicate so the call site reads
  symbolically.

### AQ-034 -- `temp_scratch` test helper belongs in shared `helpers`

- **Date:** 2026-05-17
- **Category:** Abstraction Boundaries (Low)
- **Commit context:** v0.7.0 ledgerstone improvements Batch B (clean-cache)
- **Resolution:** Moved `temp_scratch` from
  `clean_cache.rs` tests into `helpers.rs` as
  `#[cfg(test)] pub(crate) fn temp_scratch`. Future
  xtask modules with file-I/O tests can reuse the same
  pid+tid+seq-isolated scratch helper instead of
  re-implementing it.

### AQ-033 -- `dir_size` is a generic FS utility, not cache-specific

- **Date:** 2026-05-17
- **Category:** Abstraction Boundaries (Low)
- **Commit context:** v0.7.0 ledgerstone improvements Batch B (clean-cache)
- **Resolution:** Moved `dir_size` to `helpers.rs`
  alongside `fmt_bytes`. `clean_cache.rs` now imports
  the helper. Future "disk-usage" / "clean" commands
  can reuse it without copy-paste.

### AQ-032 -- `dir_size` recursion dropped failing-entry path

- **Date:** 2026-05-17
- **Category:** Error Handling & Messages (Medium)
- **Commit context:** v0.7.0 ledgerstone improvements Batch B (clean-cache)
- **Resolution:** Changed `dir_size`'s return type to
  `Result<u64, String>`. `fs::read_dir(path)?` and
  `entry?` now wrap into `format!("read_dir {path}: {e}")`
  and `format!("entry under {path}: {e}")` at the
  failure site. The recursive callers print the wrapped
  message verbatim, so warnings now name the specific
  failing path instead of the top-level entry being
  walked.

### AQ-031 -- `[profile.release]` template semantics under-documented (cross-confirmed with RT-037)

- **Date:** 2026-05-17
- **Category:** API Design (Medium)
- **Commit context:** v0.6.0 ledgerstone improvements Batch A
- **Resolution:** See RT-037. Expanded the profile comment
  to explicitly call out the optimisation tradeoffs that
  derived projects will inherit.

### AQ-030 -- `test_check_xtask` swallowed failure diagnostics (cross-confirmed with RT-036)

- **Date:** 2026-05-17
- **Category:** Error Handling & Messages (Medium)
- **Commit context:** v0.6.0 ledgerstone improvements Batch A
- **Resolution:** See RT-036. Extracted shared
  `report_failure` helper so both the CLI `test` command
  and validate's xtask-only step produce identical rich
  diagnostics on failure.

### AQ-029 -- Parallel argument-building paths in `test_cmd.rs`

- **Date:** 2026-05-17
- **Category:** API Design (Low)
- **Commit context:** v0.6.0 ledgerstone improvements Batch A
- **Resolution:** Added a `Scope` enum (`Workspace` /
  `XtaskOnly`) to `build_args` so both `test()` and
  `test_check_xtask()` go through the same arg-construction
  path. Future shared flags can be added once. New unit
  tests cover both scopes and the empty-filter error.

### AQ-028 -- `build.ps1` help block column widths drifted

- **Date:** 2026-05-17
- **Category:** Style (Low)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Normalized all help-row gaps to 14
  columns; new and old commands now line up.

### AQ-027 -- `sort.ts` hardcoded locale to "en"

- **Date:** 2026-05-17
- **Category:** API Design (Low)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** `compareNames` / `compareIds` now accept
  an optional `locale` argument; the default uses the
  runtime default locale (`Intl.Collator(undefined, …)`).

### AQ-026 -- `INSTALL_FRONTEND` tripwire could drift from the constant

- **Date:** 2026-05-17
- **Category:** Type Safety (Medium)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** The bash script now compares `$1`
  against `$2`, and `$2` is supplied by the caller as
  `deploy_config::REQUIRED_DEPLOY_PATH`. The literal
  and the constant cannot drift independently.

### AQ-025 -- `parse_port` returned `Option<String>`

- **Date:** 2026-05-17
- **Category:** Type Safety (Low)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Now `Option<u16>` with range
  validation via `.parse()`; out-of-range values return
  `None`.

### AQ-024 -- `print_final_message` returned `Result` for a cosmetic step

- **Date:** 2026-05-17
- **Category:** API Design (Low)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Made the function infallible: on read
  failure it falls through to a `?` placeholder. The
  remote is fully provisioned by this point, so the
  banner should never fail the command.

### AQ-023 -- `prompt_enter` and `ssh_test` were dead code with `#[allow(dead_code)]`

- **Date:** 2026-05-17
- **Category:** YAGNI (Low)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Deleted both. Templates shouldn't ship
  unused "extension hooks"; if a future feature needs
  them, write them with a real call site and test.

### AQ-022 -- `DeployConfig::remote()` allocated on every call

- **Date:** 2026-05-17
- **Category:** Performance (Low)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Precomputed `remote: String` once in
  `load()`; getter returns `&str`.

### AQ-021 -- `DeployConfig` fields were `pub String` (bypassed validation)

- **Date:** 2026-05-17
- **Category:** Encapsulation (Medium)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Privatized fields, exposed `rpi_host()`,
  `deploy_path()`, `remote()` getters returning `&str`.
  Dropped the unused `rpi_user` field (callers only need
  the precomputed `remote`).

### AQ-020 -- `RemoteError::NonZeroExit { cmd: &'static str }` was inflexible

- **Date:** 2026-05-17
- **Category:** API Design (Low)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Changed `cmd` to `String`; added a
  `label: &str` parameter to `ssh_run`/`ssh_capture`/
  `ssh_bash`/`scp_to` so error messages disambiguate
  which call site failed.

### AQ-019 -- `StdError::source` impl on `RemoteError` was dead infrastructure

- **Date:** 2026-05-17
- **Category:** Dead Code (Low)
- **Commit context:** v0.5.0 deploy-as-xtask port
- **Resolution:** Dropped the `source()` impl and its
  tests. Nothing at the `Result<(), String>` boundary
  walks the source chain, and `Display` already includes
  the inner `io::Error` for the variants that have one.

### AQ-018 -- tsconfig.json redeclared @tsconfig/svelte defaults

- **Date:** 2026-04-16
- **Category:** Project Configuration (Low)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** Reduced to only `noEmit: true`;
  everything else inherited from the extended base.

### AQ-017 -- template-feedback entries mixed statuses

- **Date:** 2026-04-16
- **Category:** Maintainability (Low)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** Added `[Deferred]`, `[Fixed locally]`,
  `[N/A for template]` prefixes to the 2026-04-16
  entries so triage is immediate.

### AQ-016 -- JSON type assertions trusted server blindly

- **Date:** 2026-04-16
- **Category:** Type Safety (Medium)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** `App.svelte` now throws on `!res.ok`
  and narrows results via `Partial<T>` + `??` fallbacks,
  so a 500 response or a missing field no longer
  silently produces `undefined` values in state.

### AQ-015 -- Invoke-Dev had no guard for missing node_modules

- **Date:** 2026-04-16
- **Category:** Maintainability (Medium)
- **Commit context:** v0.4.0 dev command + frontend TypeScript
- **Resolution:** `Invoke-Dev` now checks
  `frontend/node_modules` before launching the backend
  and emits an actionable error pointing at
  `cd frontend && npm install`.

### AQ-014 -- Vec<&str> limits future extensibility

- **Date:** 2026-04-15
- **Category:** API Design (Low)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Low severity, not fixed -- pragmatic
  for current usage.

### AQ-013 -- validate stops on first failure

- **Date:** 2026-04-15
- **Category:** API Design (Low)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Low severity, not fixed -- fail-fast
  is defensible since later steps may depend on earlier.

### AQ-012 -- String-based errors across all modules

- **Date:** 2026-04-15
- **Category:** Error Handling (Low)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Low severity, not fixed -- pragmatic
  for xtask scope, consistent with AQ-004 resolution.

### AQ-011 -- Option<String> error pattern in results

- **Date:** 2026-04-15
- **Category:** Type Safety (Medium)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Low severity, not changed structurally.
  Applied `match` on `Option` instead of
  `unwrap_or_default()` to eliminate masked errors.

### AQ-010 -- unwrap_or_default on known-Some

- **Date:** 2026-04-15
- **Category:** Correctness (Low)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Replaced `unwrap_or_default()` with
  `match` in `dupes()`, `run_clippy()`,
  `run_coverage()`.

### AQ-009 -- helpers.rs tests don't call step_output

- **Date:** 2026-04-15
- **Category:** Type Safety (High)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Extracted `format_step()` function,
  tests now assert on actual function output.

### AQ-008 -- Duplicated clippy argument arrays

- **Date:** 2026-04-15
- **Category:** API Design (Medium)
- **Commit context:** v0.3.0 template improvements
- **Resolution:** Extracted `CLIPPY_ARGS` constant,
  implemented `clippy()` in terms of `clippy_check()`.

### AQ-007 -- format! + parse for SocketAddr

- **Date:** 2026-04-10
- **Category:** API Design
- **Commit context:** v0.2.1 review finding fixes
- **Resolution:** Changed `cli.bind` from `String` to
  `IpAddr` (parsed by clap). Construct `SocketAddr`
  directly via `SocketAddr::new(cli.bind, cli.port)`,
  eliminating the fallible `format!` + `.parse()` +
  `.expect()` chain.

### AQ-006 -- `create_router` accepts `&str` not `&Path`

- **Date:** 2026-04-10
- **Category:** API Design
- **Commit context:** v0.2.1 review finding fixes
- **Resolution:** Changed `frontend_path` parameter
  from `&str` to `&Path`. Uses `Path::join` instead of
  `format!` for index path. Updated `cli.frontend` to
  `PathBuf` and all test call sites.

### AQ-005 -- Inconsistent String vs &'static str

- **Date:** 2026-04-10
- **Category:** Type Safety
- **Commit context:** v0.1.2 template feedback fixes
- **Resolution:** Changed all response struct fields
  that hold compile-time-known values to
  `&'static str`. Simplified all handlers to return
  `Json<T>` directly instead of
  `(StatusCode::OK, Json(...))` tuple since 200 is
  the Axum default.

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
