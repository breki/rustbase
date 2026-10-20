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

- **Configuration is small.** The cooldown prototype needed three
  values: the workspace root, the allow-list env-var name and the
  registry user agent. (`Config` in `crates/rustbase-xtask/src/lib.rs`
  now holds every gate's settings.)
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
`worktree`, hoard's `cross`), extra `validate` steps (jutro's
architecture lint), and for deploy, extra config keys and
hooks before install, before start and during setup. The library
provides the step and subcommand types; projects implement them.
For `validate` and the subcommands this is in place (#8):
`step::standard` is the library's gate list in order, and a project
inserts its own steps before `step::run_all` runs them;
`cli::LibCommand` is the library's subcommands, which a project's
CLI flattens into its own and dispatches with `cli::run`. Deploy's
hooks are #10.

### Distribution and versioning

Published on crates.io as `rustbase-xtask` (#4); 0.1.0 went out on
2026-10-20, tagged `rustbase-xtask-v0.1.0`. Through crates.io the
licence/sources gate (`deny.toml` rejects git sources) and the
cooldown and audit tooling apply to it like any other dependency. It
follows SemVer with its own version, in
`crates/rustbase-xtask/Cargo.toml`, separate from the template's.
Upgrading it waits out the 14-day cooldown like any dependency; a
project that wants a fix sooner allow-lists that one version with a
stated reason.

This template depends on it by path and version (`xtask/Cargo.toml`):
inside this repository cargo builds the local source, so a library
change and the template change using it land together. The
`version` is the last release; `main` can hold library changes not
published yet. A downstream moves to the published crate by the
steps under "Migrating a downstream" below.

Releasing the library is manual for now, and only the operator can
run the publish, which needs their crates.io login. Run it on `main`
after the library change has merged, so the published source and the
tag sit on a commit `main` keeps. The step-1 commit is a direct
bookkeeping commit, like `/release`'s, not one made through
`/commit`:

1. Bump `version` in `crates/rustbase-xtask/Cargo.toml` by SemVer.
   The changes to weigh are the commits since the last
   `rustbase-xtask-v*` tag that touch the library
   (`git log rustbase-xtask-vX.Y.Z.. -- crates/rustbase-xtask`). A
   bump that leaves the range of the requirement `xtask/Cargo.toml`
   puts on the library (`version = "0.1"`) -- a minor bump while the
   version is `0.x`, or a major one -- also raises that requirement
   to the new `0.y` (or `x`); otherwise cargo stops every command
   with "failed to select a version for the requirement". Run
   `cargo check -p rustbase-xtask` so `Cargo.lock` takes the new
   version, then commit the manifests and `Cargo.lock` together.
2. `cargo publish -p rustbase-xtask --dry-run`. `git status` must
   still be clean afterwards; if it is not, commit what changed
   (normally `Cargo.lock`) and repeat this step.
3. `git tag -a rustbase-xtask-vX.Y.Z -m "rustbase-xtask vX.Y.Z"`. The
   prefix keeps the tag out of the deploy guard's and `release.yml`'s
   `v*` patterns, which mean a template release.
4. `cargo publish -p rustbase-xtask`. A published version can be
   yanked, never deleted. If it fails, delete the tag (`git tag -d`;
   it has not been pushed), fix the cause and repeat from step 2.
5. Push the commit and the tag.

Its own changelog and an automated release are a follow-up
(`release-library` in `docs/todo.md`).

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
| `validate` | step runner; each gate supplies its `step()` (done, #5). The runner, the `Step` type and the standard gate list moved into the library as `step` (#8); a project inserts its own steps (here, the frontend gates) |
| `frontend*` | library-ready, optional feature |
| `deploy*` | last; generic pieces (remote run, tag guard, staging, crash check) only, with hooks |
| `backfeed`, `feedback`, `sync` | template-maintenance tools; stay in the template |

Downstream-only modules, evaluated for the library (#8). Each uses
only std and `serde_json` beyond its project's own helpers.

| Module | Lines | Verdict | What ties it to its project |
|-|-|-|-|
| bombyx `licenses` (third-party licence file) | 1,073 | moved (#31) as `licenses`, run in CI as a check | the product name, now `Config::name` |
| bombyx `version_sync` (`<!-- version: X -->` sentinels) | 204 | moved (#30): `version_sync`, `version-sync`, run by `/release` | the manifest path, now `Config::version_manifest` |
| hoard `repo_hygiene` (scripts staged executable) | 231 | moved (#28): `scripts`, the Scripts step | nothing: the `scripts/` convention |
| jutro `tag_signature` (SSH-signed release tags) | 409 | moved (#32) as `tag_signature`, `verify-release-tag`; `/release` signs and runs it when `.github/release-signers` is tracked | placeholder strings naming jutro, now generic |
| jutro `xtask_log` (tee output to `target/<name>.log`) | 421 | moved (#29) as `tee`, used by `validate`; its cargo-streaming variants stay in jutro | nothing |
| jutro `ports` (`.ports` reader) | 312 | keep (#33): nothing outside jutro would call it. The template's `.ports` has four keys, dev and e2e (`.ports.sample`), read from shell, PowerShell and TypeScript (`scripts/lib/port-utils.sh`, `build.ps1`, `playwright.config.ts`, `frontend/vite.config.js`), not Rust; no other downstream reads it from Rust (checked 2026-10-20) | its callers, all jutro subcommands (`worktree` among them), and a layout of only the two dev keys |
| jutro `worktree` (a worktree with free ports) | 573 | keep | the `jutro-` prefix, `ports`, `vite`, npm |
| hoard `systemd_unit` (unit-file parser) | 435 | keep; too niche | its embedded unit file and paths |

A moved module takes its tie as a parameter: the `Config` field in
the last column, where there is one. The parser at the core of
`systemd_unit` (sections, comments, continuations; about 100 lines)
is generic and can follow once a second project ships a unit.

## Migrating a downstream

clockdump went first (#9, breki/clockdump#1), from a vendored copy of
the template's `xtask` at v0.17.0 to `rustbase-xtask` 0.1.0. Its
`validate` passes every gate it ran before, coverage at 96.9%, and
`xtask` came to about -1,100 lines net -- less than the one-module
prototype's -1,773, because step 1 copies in the template's deploy
and frontend modules, compiled out but present. kozmotic followed
(breki/kozmotic#1): it adopted three gates it had never run (step 4),
its audit found 10 TLS advisories to fix, and its `xtask` grew by
about 960 lines, since it had vendored fewer gates than the template
carries.

**Before step 1**, bring everything outside `xtask/` up to the
template with `/template-sync`. Its rules still skip `xtask/`; step 9
changes that. When that sync is large, it can wait as its own change
(kozmotic's did): step 9 then covers only the canon that describes
`xtask`. The steps, in the order clockdump took them:

1. **Copy the template's `xtask/src`** from the commit tagged
   `rustbase-xtask-vX.Y.Z` for the version you will pin, not from
   `main`, which can call library code that version lacks. Its
   `main.rs` flattens the library's `cli::LibCommand` into its own
   `XCommand`, and `validate.rs` runs the library's
   `step::standard` gate list, so the downstream's vendored gate
   modules go and nothing replaces them. Take every file, the
   deploy, frontend and backfeed modules included: `main.rs` and
   `sync.rs` call into them, and `cargo xtask clippy` compiles each
   feature group. A module of the project's own (hoard's deploy
   pipeline, jutro's `worktree`, an architecture lint) stays: its
   subcommand becomes an `XCommand` variant in `main.rs` beside the
   flattened `LibCommand`, and its gate a step placed with
   `step::insert_after` in `validate.rs` ("Optional xtask features"
   in `CLAUDE.md`). `main.rs` is then merged too, like the two files
   in step 9. clockdump had no such module, so this part has not
   been run yet.
2. **`xtask/Cargo.toml`**: take the template's, then leave the
   feature groups the project does not use out of `default` and drop
   the `path` from the library dependency, leaving
   `rustbase-xtask = { version = "0.1", features = ["clap"] }` (the
   downstream has no copy of the crate). The rest of the file is
   needed: `clean-cache` names the optional `windows-sys`
   dependency, and the Deny gate wants the `license` field.
3. **`helpers.rs`**: `Config::for_project("<name>", ...)`, which keeps
   the `<NAME>_DEP_AGE_ALLOW` variable. Drop the template's canon
   exemptions, which name template paths, and the test that asserts
   them; keep `audit_npm_dir` only with a frontend.
4. **`validate.rs`**: the gates the old vendored `validate.rs` did
   not run can be left out, so the migration keeps the gates the
   project had. Compare its steps with `step::names` in
   `crates/rustbase-xtask/src/step.rs`, write the difference as a
   local list of `step::names::*` constants (clockdump's `DEFERRED`:
   Canon, Records, Scripts, Doc, Deny), and remove them with
   `steps.retain(|s| !DEFERRED.contains(&s.name()))` **after** any
   `insert_after`: the template anchors the frontend gates on Deny,
   so filtering first leaves no anchor and `validate` fails at
   startup. Adopt the left-out gates later, one change each, or
   some now: kozmotic adopted Dep-age and Audit, which needs the CI
   job in step 7 and a fix for whatever the audit finds, and Scripts,
   since its release job runs a script by path.
5. **Coverage exclusions** a vendored `coverage.rs` hard-coded move
   to `[workspace.metadata.coverage] ignore` in the root
   `Cargo.toml`.
6. **`clippy.toml`**: the first `cargo xtask clippy` names each
   `doc-valid-idents` entry the template's `xtask` needs (clockdump
   lacked `CommonMark`); add them.
7. **The cooldown**: a library version inside the 14 days needs the
   allow-list, with a stated reason, on each local run
   (`<NAME>_DEP_AGE_ALLOW=rustbase-xtask@X.Y.Z cargo xtask validate`)
   and, where the project has CI, in a repository variable of the
   same `<NAME>_DEP_AGE_ALLOW` name read by its dep-age job. A
   project whose CI has no such job takes the template's (the `dep-age`
   job in `.github/workflows/ci.yml`), and the variable must be set
   before the migration's own PR runs it. It can go once the 14 days
   pass.
8. **Run `validate`.** Failures in the project's own code from a
   newer toolchain (clockdump: Clippy 1.99's `assert_is_empty`) are
   their own commit.
9. **Bring the project's canon in step**: docs naming the deleted
   modules (`coverage.rs`'s regex), its `template-feedback.md` and
   `/template-sync` rules where they say to skip `xtask` files (every
   file is now synced: the template's verbatim, `helpers.rs`,
   `validate.rs` and any file holding project modules merged), and
   its `CLAUDE.md` gate list. Three changes in behaviour reach past
   the docs:
   - The library's `validate` formats files in place unless given
     `--check`. A `/commit` or `/release` that validates after
     staging needs `--check`, or it fixes the working tree while the
     unformatted copy is committed.
   - CI that calls `cargo test` / `cargo clippy` directly misses the
     per-feature lint passes `validate` requires; it calls
     `cargo xtask test` and `cargo xtask clippy` instead.
   - A todo list in another layout (kozmotic's `TODO.md`, Pending and
     Done) moves to `docs/todo.md` in the library's, since `cargo
     xtask todo` reads only that; done items leave the file.

The other downstreams (bombyx, hoard, jutro) follow the same steps.

## Proposed follow-up issues

1. Settle the crate name and publish `0.1` (done: #4, `rustbase-xtask`
   0.1.0, 2026-10-20).
2. Decouple feature selection: registered `validate` steps, helpers
   grouped by user (done: #5).
3. Extract `audit`, `deny`, `coverage`, `dupes`, `doc_cmd`, `canon`,
   taking the downstream fixes listed above (done: #6).
4. Extract the output parsers (`check`, `clippy_cmd`, `test_cmd`,
   `fmt_cmd`) and `changelog`/`todo` (done: #7; `todo` uses bombyx's
   layout only).
5. Extension points for project subcommands and steps (done: #8;
   deploy hooks are #10).
6. Migrate clockdump (done: #9, breki/clockdump#1), then the other
   downstreams.
7. Deploy: decide what, if anything, is shared.
