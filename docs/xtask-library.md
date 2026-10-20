# Shared xtask library: recommendation

Investigation for breki/rustbase#2. Question: can the template's
`xtask` tooling become a library that rustbase-derived projects
depend on, so a fix ships once as a version bump instead of being
merged by hand into every project's copy?

**Recommendation: yes, proceed.** Extract the gates and doc tooling
into a crates.io library behind each project's thin `xtask`, one
module at a time, starting from the dependency-cooldown prototype on
branch `feat/xtask-lib-dep-age`. Make feature selection work without
code edits before extracting the coupled modules (done in
breki/rustbase#5). Leave deploy project-owned until last.

## Evidence

### What downstream drift actually is

Each downstream's `xtask/src` was diffed file by file against the
template (and against the template at its recorded sync point, to
separate "behind" from "changed locally"), and every difference was
classified. Measured 2026-10-19.

| Downstream | Absorbed by a configurable library | Drift is mostly | What remains |
|-|-|-|-|
| clockdump | ~100% | stale; deploy/frontend removed | nothing |
| kozmotic | ~95% | stale; features removed | two coverage fixes to upstream |
| hoard | ~80% | stale (selective sync) | its deploy pipeline |
| bombyx | ~75% | removals, cosmetic, stale | its records-format `todo` |
| jutro | ~65%, plus improvements | stale since May; new local work | deploy pipeline, a project lint |

Three conclusions follow:

- **Renaming is not the problem.** Rename/config hunks are 0.5-5% of
  each project's drift. The bulk is staleness and removed features,
  which a versioned dependency and feature selection remove.
- **The current model loses improvements.** About 40 downstream fixes
  and a dozen generic downstream-only modules never reached the
  template (examples in the issue #2 step-1 comment). In a library
  they would be written once.
- **Project-specific code is concentrated** in deploy (hoard, jutro),
  bombyx's records-based `todo`, and jutro's architecture lint.

### What the prototype showed

The cooldown gate (`dep-age`, `dep-age-check`, `dep-preflight`) moved
into `crates/rustbase-xtask` with the template's `xtask` calling it.

- **Configuration is small.** Three values: the workspace root, the
  allow-list env-var name and the registry user agent.
  `Config::for_project("hoard", root)` derives `HOARD_DEP_AGE_ALLOW`
  and `hoard-xtask-dep-age`, the renames downstreams made by hand.
- **The coverage gate starts to apply.** `xtask` is exempt from the
  90% gate; a library crate is not. The moved code first measured
  79.8%. Splitting the network and process calls (curl, `git show`,
  `cargo update`, the clock) into an excluded `io.rs`, per the
  "Coverage exceptions for hardware-bound code" recipe in
  `CLAUDE.md`, and testing the entry points against a throwaway git
  repository raised it to 91.4%. Every extracted module will need
  this split. The leaf can belong to one module (`dep_age/io.rs`)
  or be shared: `audit` and `deny`, extracted later (#6), share
  `process.rs`. Each leaf is listed under
  `[workspace.metadata.coverage]` in the root `Cargo.toml`.
- **The gate also applies per file.** Each file must reach 85% as
  well as the workspace reaching 90%. `crates/rustbase-xtask/src/deny.rs`
  first measured 75% with the crate at 92%. Its entry point only
  hands the excluded spawn its arguments, so tests never call it,
  and each line of a multi-line argument list inside it counted as
  uncovered. A `const` item has no lines to count, so the list
  moved into one. Coverage runs late in `validate`, so running
  `cargo xtask coverage` alone first finds such a failure sooner.
- **The doc gate applies to the public API.** A module doc that
  linked a private submodule was fine in a binary and fails in a
  library.
- **Adoption is cheap.** In a copy of clockdump, replacing its
  cooldown module with the library removed 1,773 lines and added 26;
  its commands and tests pass, and it gained the three months of
  cooldown fixes it had missed.

## Design

### Shape

A library crate, called by each project's own `xtask` binary. The
project keeps `cargo xtask ...`, its CLI, and any project-specific
subcommand or gate; the library owns the gates. `clap` support is an
optional feature (`features = ["clap"]`) so the library does not
force a CLI framework.

### Configuration

One `Config` value, built by the project's `xtask`, extended per
module. Defaults derive from the project name; anything a project
sets in data rather than code goes in `[workspace.metadata.<lib>]`
in the root `Cargo.toml`, which `coverage` already reads for its
ignores. Knobs the survey found real projects need:

- project name (env-var prefix, user agent, temp-dir prefix);
- primary crate manifest path (version source, deploy guard);
- feedback-file sections (the todo file is fixed: `docs/todo.md`, one
  layout);
- thresholds: coverage overall/file, duplication pooled/per crate,
  module size, the cooldown window;
- strictness of the network gates (fail or warn when they cannot
  finish);
- canon-check exemptions;
- which gates `validate` runs, and in what order.

### Feature selection without code edits

The template's `main.rs`, `validate.rs` and `helpers.rs` referenced
every module directly, so dropping a feature meant editing shared
files; kozmotic carries ten `#[allow(dead_code)]` helpers just to
compile.

Done in breki/rustbase#5. The optional groups (`frontend`,
`deploy`, `template-tools`, `clean-cache`) are Cargo features of
`xtask`. Each gate builds its own `validate` step, and each group's
helpers live in the group. `cargo xtask clippy` lints every feature
off and each one alone, so dead code from a dropped group fails the
gate (`CLAUDE.md`, "Optional xtask features"). Cargo features were
chosen over a step registry a project edits, because dropping a
group then needs no code edit and the result is lint-checked. The
library can expose the same groups as its own optional features.

### Extension points

Projects need to add, not only configure: extra subcommands (jutro's
`worktree`, hoard's `cross`), extra `validate` steps (bombyx's
records check, jutro's lint), and for deploy, extra config keys and
hooks before install, before start and during setup. The library
provides the step and subcommand types; projects implement them.

### Distribution and versioning

Published on crates.io, so the licence/sources gate (`deny.toml`
rejects git sources) and the cooldown and audit tooling apply to it
like any other dependency. It follows SemVer, released by its own
`/release`. Upgrading it waits out the 14-day cooldown like any
dependency; a project that wants a fix sooner allow-lists that one
version with a stated reason.

### What still needs agent-driven sync

`CLAUDE.md`, `.claude/commands/`, `.claude/agents/`, CI workflows and
config files are prose and data, not Rust. The library removes the
largest and most error-prone part of `/template-sync` (merging
diverged code) but not the sync itself. Shipping command files with
the library is a possible later step, not part of this plan.

## Module classification

| Module | Verdict |
|-|-|
| `dep_age` (+ gate, preflight) | extracted (prototype) |
| `audit`, `deny` | extracted; `audit` takes a `Strictness` (fail or warn when it cannot finish), `deny` none: a missing tool always fails |
| `coverage`, `dupes` | extracted; limits are `Config::thresholds`; kozmotic's coverage hardening (unreadable manifest, vacuous run) and jutro's pooled-plus-per-crate duplication limit taken (copies between crates are not measured, by design: on jutro they came to 0.1% of the lines, and one scan of all crates together would have to reproduce how code-dupes picks files -- links, dot and `target` directories, per-crate ignore lists and config; #15) |
| `doc_cmd`, `canon` | extracted; canon exemptions are `Config::canon` (`canon::Rules`), set in `tasks_config()` (`xtask/src/helpers.rs`) |
| `check`, `clippy_cmd`, `test_cmd`, `fmt_cmd` | extracted (#7) as the library's `check`, `clippy`, `cargo_test` and `fmt`: one `Failure` report per gate, a tail of the output when nothing else is found, one shared cargo-diagnostic filter; bombyx's drop of per-target `could not compile` lines and hoard's single-report `check` error taken |
| `changelog`, `todo` | extracted (#7): `changelog` with jutro's `infer-bump` / `promote`; the markdown helpers as `markdown` with hoard's hardening (CRLF-preserving rejoin, sections found by column-0 headings only, structure-preserving wrap); `todo` in bombyx's layout (one `### <slug>` entry per item, removed when done), the only layout: every attempt to allow other headings among the items let an edit misplace or cut text, so the file is an introduction then items only |
| `records` (bombyx's `records`) | extracted (#21) as the library's `records`: one parser for every record file (`todo` reads its items through it) and the `records-check` gate; the files are `Config::record_files`. It checks an id in an entry's `Depends on` / `Supersedes` field; canon-check checks a backlog ID cited in canon prose |
| `clean_cache`, `helpers` | helpers grouped by the module that uses them (done, #5); Windows-only code stays behind `cfg` |
| `validate` | step runner; each gate supplies its `step()` (done, #5) |
| `frontend*` | library-ready, optional feature |
| `deploy*` | last; generic pieces (remote run, tag guard, staging, crash check) only, with hooks |
| `backfeed`, `feedback`, `sync` | template-maintenance tools; stay in the template |

Downstream-only modules worth adding to the library: bombyx's
third-party licence generator and version-sentinel sync, hoard's
systemd unit parser and script-executable check, and jutro's tag
signature check, `.ports` reader, worktree setup and log tee.

## Migration plan (one downstream: clockdump)

1. Publish the library with the cooldown module.
2. In clockdump: add the dependency, delete its vendored
   `xtask/src/dep_age.rs` and `xtask/src/dep_age/`, and call the
   library with `Config::for_project("clockdump",
   workspace_root())` where the template does: `main.rs` and the
   `dep_age_gate.rs` step. (The prototype run, made before the
   step moved out of `validate.rs`, came to -1,773 / +26 lines.)
3. Run `cargo xtask validate` and its CI; record the result.
4. Repeat per module as each is extracted; the other downstreams
   follow once their copies are synced to a template version that
   matches the library's.

## Proposed follow-up issues

1. Settle the crate name and publish `0.1` with the cooldown module.
2. Decouple feature selection: registered `validate` steps, helpers
   grouped by user (done: #5).
3. Extract `audit`, `deny`, `coverage`, `dupes`, `doc_cmd`, `canon`,
   taking the downstream fixes listed above (done: #6).
4. Extract the output parsers (`check`, `clippy_cmd`, `test_cmd`,
   `fmt_cmd`) and `changelog`/`todo` (done: #7; `todo` uses bombyx's
   layout only).
5. Extension points for project subcommands and steps.
6. Migrate clockdump, then the other downstreams.
7. Deploy: decide what, if anything, is shared.
