# Changelog

All notable changes to this project will be documented
in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `cargo xtask changelog add` and `cargo xtask todo` (list/add/done) commands
  that edit CHANGELOG.md and docs/todo.md mechanically, so `/commit`, `/todo`,
  and `/implement` no longer hand-edit those files. `docs/todo.md` is an
  introduction, then one `### <slug>` entry per item with a one-line
  `**Summary:**` (the layout of the downstream project bombyx), and `todo done
  <slug>` removes the entry (git history records what shipped; there is no Done
  section). Both live in the `rustbase-xtask` library (#7). `/template-sync`
  never copies `docs/todo.md`, so a derived project keeps its old `## Pending`
  / `## Done` file, and the command refuses it until each pending bullet is
  converted to an `### <slug>` entry with a `**Summary:**` line.
- cargo xtask deny -- an offline cargo-deny licence/bans/sources gate configured
  by deny.toml, wired into validate and every-push CI (a missing cargo-deny is a
  hard error, not a warning).
- GitHub Actions workflows: ci.yml (test across Windows/Linux/macOS, fmt,
  clippy, deny, frontend gates) and release.yml (coverage, duplication and a
  standalone audit on tag).
- /template-improve sweep mode: run with no argument to mine commits since the
  feedback file was last touched for template feedback, instead of only logging
  what the operator volunteers.
- `/review`: three reviewers in sequence (Artisan, Red Team, and a new Fresh
  Reader agent), one lane each, against an immutable `target/` snapshot, with a
  behaviour-defect stopping rule and a three-round cap. It commits nothing.
- `cargo xtask doc`: rustdoc with `-D warnings` in a public and a private pass,
  so a broken doc link fails `validate` and the Stop hook instead of building
  clean.
- `cargo xtask canon-check`: a markdown-only `validate` gate that fails on canon
  prose naming a missing path, an unresolved cross-reference, an ungranted `git`
  subcommand, an over-wide line, or a backlog ID cited in no backlog.
- `/issue`, `/ask`, `/rundown` and `/short` commands.
- CI runs the 14-day dependency cooldown: a `dep-age` job in `ci.yml` runs
  `cargo xtask dep-age-check --base <ref>` against the commit the push or pull
  request started from, failing on a fresh dependency, an unreachable registry
  or a missing base; its allow-list comes from the maintainer-set
  `RUSTBASE_DEP_AGE_ALLOW` repository variable. `dep-age-check` gained `--base`
  (default `HEAD`).
- `cargo xtask dep-age <eco> <pkg> --latest-aged --within <line>` keeps the pin
  target on one release line (`3`, `3.x`, `0.4`), so it can no longer suggest a
  major the tree cannot use (#1).
- A `crates/rustbase-xtask` library crate now holds the
  dependency-cooldown gate; the template's `xtask` calls it with a per-project
  `Config` (`Config::for_project` derives the `<NAME>_DEP_AGE_ALLOW` variable
  and registry user agent). `docs/xtask-library.md` records the investigation
  and recommendation (#2).
- The repo-root `package-lock.json` of the Playwright E2E harness is now
  committed (it was git-ignored) and checked by the dependency-cooldown gate;
  `CLAUDE.md` documents the one-time `npm ci` + `npx playwright install
  chromium` setup.
- Optional `xtask` command groups as Cargo features (`frontend`, `deploy`,
  `template-tools`, `clean-cache`, all on by default): a project drops one from
  `default` in `xtask/Cargo.toml` with no code edits, and its `validate` steps
  and `[N/M]` count follow. `cargo xtask clippy` also lints `xtask` with every
  feature off and each one alone, so a dropped group leaves no dead code.
  `validate` refuses to run while `frontend/` exists with the `frontend`
  feature off, since none of its gates would run.
- The `rustbase-xtask` library now holds the `audit` and `deny` gates too.
  `audit` takes a `Strictness` (fail or warn when the check cannot finish);
  `deny` takes none, since a missing cargo-deny always fails (part of #6).
- The `rustbase-xtask` library now holds the `coverage` and `dupes` gates; their
  limits are `Config::thresholds` (coverage 90% overall and 85% per file,
  duplication 6% pooled across crates and 12% per crate) by default; a project
  overrides them in `tasks_config()` (part of #6).
- The `rustbase-xtask` library now holds the `doc` and `canon` gates, completing
  the gate extraction (#6). Canon-check's exemptions are configuration:
  `Config::canon` carries the paths that are absent on purpose (each with a
  reason), the files whose backlog-ID citations are not checked, the repo path
  prefixes and the column limit, set in `tasks_config()`. An exemption covers
  its exact path, or a folder when it ends in `/` (`crates/a` does not cover
  `crates/a-web/...`). An entry that is blank, has no reason, names a
  checked folder itself, or lies outside every checked folder fails the gate,
  as does a repo prefix that does not end in `/`.
  The library also exports its cargo-output and ISO-date helpers as
  `rustbase_xtask::text`.
- The `rustbase-xtask` library now holds the `check`, `clippy`, `test` and `fmt`
  gates (part of #7), each returning a `rustbase_xtask::Failure` (a summary plus
  diagnostic lines). One cargo-diagnostic filter,
  `rustbase_xtask::text::is_cargo_diagnostic`, serves clippy, doc and the test
  gate's compile errors.
- `cargo xtask changelog infer-bump` prints the SemVer level the `[Unreleased]`
  bullets call for, and `cargo xtask changelog promote --version X.Y.Z --date
  YYYY-MM-DD` dates `[Unreleased]` and opens a fresh one (from jutro);
  `/release` now calls both instead of doing the work in prose. The
  `rustbase-xtask` library holds `changelog` and a public `markdown` module
  (part of #7).
- `cargo xtask records-check`, also a `validate` step: checks the record files
  (`docs/todo.md`, the three reviewer logs and
  `docs/developer/template-feedback.md`) for an id used twice, an id that is not
  kebab-case starting with a letter (so a bare date is refused; `todo` slugs
  follow the same rule), an unknown field label, and a `Depends on` /
  `Supersedes` id that names no entry; a configured file that does not exist is
  skipped and named.
  The 37 template-feedback entries whose headings had no id now carry a
  `tf-<date>-<slug>` id, dated from the heading or, when it had none, from when
  the entry first appeared in git. A derived project whose feedback file still
  has headings without a `tf-` id (`### 2026-05-18 -- title`, or a title alone)
  converts them once by hand, the one exception `/template-improve`'s rules
  allow to that file's no-hand-edit rule. `feedback-add` now refuses a body the
  check would reject (one opening with an unknown `**Label:**`, holding a
  column-0 `### ` line outside a code block, or leaving a code block open), and
  `todo list`/`add`/`done` refuse any `###` heading in `docs/todo.md` that is
  not an item.
- `cargo xtask scripts-check`, also a `validate` step (Scripts, after Records):
  fails when a file under `scripts/` that starts with `#!` is not staged
  executable (mode `100755` in the git index), since canon runs those by bare
  path (`/commit` runs `scripts/e2e.sh`) and the shell refuses a `100644` one
  with "permission denied" (exit 126). Taken from the hoard downstream project.
- `cargo xtask validate` also writes its step lines and a failure's report to
  `target/validate.log`, rewritten each run, so the record survives a closed
  terminal or a caller that shows only the tail. A second `validate` in the
  same checkout is refused while the first runs. It uses the library's new
  `tee::Tee`, which any xtask command can use (`validate` is the only one so
  far); taken from the jutro downstream project.
- `cargo xtask version-sync`, which `/release` now runs during the bump: sets
  every `<!-- version: X -->` sentinel line in `README.md` and `docs/`, and the
  number in any `VERSION=<number>` line in the same file (the rest of that line
  kept; `export VERSION=` and `VERSION=v1.0` are not touched), to the version
  `cargo metadata` reports for `Config::version_manifest` (default
  `crates/<project>/Cargo.toml`). Only a line holding the sentinel alone,
  outside a fenced code block, opts a file in; each line keeps its own ending,
  and a failure to read any doc writes none. It lists every file with a
  sentinel, which `/release` stages. Taken from the bombyx downstream project.
- `cargo xtask licenses` writes `THIRD-PARTY-LICENSES`: every Rust crate that
  goes into building the distributed binaries (or one member's, with
  `--package`), with its licence texts, read from the crate sources already on
  disk. It goes in cargo's target directory unless `--out` names a file or a
  directory. It fails when a crate ships no licence text (`--max-missing`
  allows some, with a stated reason). CI runs it on every push for the Linux,
  Windows and macOS targets as an early warning, since this template deploys
  rather than ships binaries; a project that ships them adds it to its release
  job. `Config` gains `name`, the project name the file names. Taken from the
  bombyx downstream project.
- `cargo xtask verify-release-tag <tag>` checks that a release tag carries an
  SSH signature from a key listed in `.github/release-signers`. It refuses an
  unsigned or lightweight tag, a tag signed any way other than SSH, and a signed
  tag moved under another name (an old release passed off as a new one). It is
  opt-in: when a project tracks that file, `/release` creates the tag with
  `git tag -s` and runs the check; without it `/release` tags as before. Deploy
  does not re-check the signature. Taken from the jutro downstream project.
- The shared xtask library is ready to publish on crates.io as `rustbase-xtask`
  0.1.0: publishing metadata, its own README and LICENSE, and a release
  procedure in `docs/xtask-library.md`. The template depends on it by path and
  version, so library and template changes land together here and downstream
  projects pin the version. Its growing types (`Config`, `Thresholds`,
  `Strictness`, `canon::Rules`, `cli::LibCommand`) are `#[non_exhaustive]`, and
  the `text` and `markdown` modules are hidden from the docs and not covered by
  SemVer.
- `cargo xtask validate --skip <gate>` leaves a gate out of a run, by its name
  or subcommand (repeatable). The run names the skipped gates at its start
  and end, a name matching no gate or a set leaving none is refused, and
  skipping Coverage still runs every crate's tests once, as a
  `Test (workspace)` step in its slot.
- rustbase-xtask `frontend` feature (off by default): the four frontend gates --
  Prettier, svelte-check, jscpd and Vitest, each running one of the project's
  npm scripts -- as `frontend::steps` for `validate` to place after Deny, and
  their `frontend-*` subcommands as `cli::FrontendCommand`, flattened into
  `LibCommand`. The folder is the new `Config::frontend_dir`: a derived project
  with a frontend sets `cfg.frontend_dir = Some("frontend".into())` in its
  `tasks_config()`, or `validate` refuses to run (unset, the gates report
  "skipped: frontend_dir not set"). The template's xtask `frontend` feature
  now enables the library's, and its five `xtask/src/frontend*.rs` copies are
  gone (#44).
- rustbase-updater, a library crate holding the self-upgrade engine jutro and
  silkboard each carried a copy of: release selection, a durable upgrade
  transaction, recovery, retry, the versioned state file and the command layer.
  Each application declares its policy type, adapter id, usage and legacy-state
  rule once in an App trait (#58).
- A committed .dep-age-allow file waives a fresh dependency for the cooldown
  gate (one name@version per line, # comments). The gate reads it as committed
  at the base it diffs against -- HEAD locally; in CI the pull request's base,
  or for a push to main the tip before the push -- so a waiver added in the same
  change waives nothing, and the waiver
  stays on record in git. Malformed lines are listed when a run fails anyway,
  never failing a run on their own. The RUSTBASE_DEP_AGE_ALLOW variable still
  works (#62).
- /security-upgrade: patch what cargo xtask audit flags, waiving a still-fresh
  fix through .dep-age-allow (landed on the base branch first), then judge from
  the deployed release whether production is exposed and file any hardening of
  our own code separately (#62).
- `cargo xtask fmt --check` checks formatting read-only, outside `validate`, and
  `cargo xtask frontend-test <filters>` runs only the frontend test files whose
  path contains a filter (vitest options go after `--`); given filters or vitest
  options, a run that finds no test file at all fails. Both come from the
  downstream project jutro, which lost them when it moved onto `rustbase-xtask`
  (#68).

### Changed

- `Config::audit_npm_dir` is now relative to the root, like the other paths in
  `Config`; an absolute path still works.

- Code reviewers (Red Team, Artisan) are now first-class read-only agents under
  `.claude/agents/`, spawned by `subagent_type`; `/retrospect` findings gained
  explicit Short/Long/Fix fields.
- cargo xtask check now compiles all targets (--all-targets), so a broken test
  call site (an altered signature) is caught by check rather than surfacing
  later under test.
- `/release` now runs a cross-platform clippy on the claimed platforms
  (Windows, Linux, macOS) other than the host, and a standalone `cargo xtask
  audit`, before the tag, because validate lints only the current platform and
  its audit degrades to a warning.
- `/commit` no longer runs the reviewers; reviewing is the separate `/review`
  command. `/implement` likewise hands off to `/review` instead of pre-launching
  reviewers.
- `/implement` and `/issue` treat `docs/issues/<slug>.md` as a working document:
  durable decisions are promoted to a reference doc or code comment and the
  planning doc is removed when the item is done.
- Command files invoke `cargo -q xtask ...`, so cargo compile progress no longer
  precedes a command's output; their `allowed-tools` grant both spellings.
- `validate`'s dependency-cooldown step fails, instead of warning, when the
  check cannot finish -- an unreachable registry, a missing `HEAD` baseline, an
  unreadable lockfile, or an npm lockfile it cannot parse (conflict markers, a
  v1 lockfile), which used to read as "no dependency changes" and pass -- in
  every run, with no variable to switch it (#1, #47). With no `HEAD` at all the
  error says to commit first, and `validate --skip dep-age` leaves the gate out.
  An unchanged lockfile, whatever its format, still needs no network and
  passes.
- `/update-deps` resolves npm `ERESOLVE` with an escalation ladder (scoped
  installs, a hand-edited range, `--legacy-peer-deps`, then a full reset)
  instead of resetting the lockfile first, which re-floated every transitive
  into the cooldown; it also documents exact pins, scoped `overrides` and an
  audit-vs-cooldown recipe (#1).
- `cargo xtask coverage` names the least-covered files and their uncovered line
  ranges when the overall threshold fails, instead of printing only the total.
- `/issue` runs `/review` before the first push: commit, review, commit the
  fixes, then push and open the PR, so the PR never shows the unreviewed state.
  It also runs `rustup update` before `cargo xtask validate`, so local Clippy
  matches CI's, and says how to fix a CI failure the change caused.
- `cargo xtask audit` errors now stand alone: a check that could not finish
  names its reason instead of "see warnings above", and a vulnerability report
  also names any source that did not finish (an unreachable npm registry used to
  go unmentioned).
- `cargo xtask dupes` holds the duplication found in each crate, pooled across
  crates (duplicate lines over total lines), to 6% and any one crate to 12%,
  instead of each crate to 6%: one copied block in a small crate no longer fails
  the gate. Copies between crates are still not measured. It reads `code-dupes`
  JSON instead of streaming its report, fails when it counted no lines at all,
  and a failure prints the command that lists the duplicates.
- `cargo xtask coverage` fails when the root `Cargo.toml` cannot be read,
  instead of falling back to the built-in ignore list (which reported the
  excluded files as a coverage drop), and fails when a run measured no files at
  all.
- A failing `cargo xtask check`, `clippy`, `test` or `doc`, or the Fmt step of
  `validate --check`, prints its report once, as the error itself, instead of a
  FAILED block followed by a second `xtask error:` line; in `validate` the
  diagnostics now follow the step line. `check` no longer counts, and `test` no
  longer lists, cargo's per-target `could not compile` lines, so one error
  reads as one error. A failure whose tool printed nothing the gate recognises
  shows the end of the tool's output instead of an empty report, and `test`
  names every failing test. A read-only format check also reports a file that
  does not parse, beside the diff.
- The `validate` runner, its `Step` type and the standard gate list moved into
  the shared library (`step::standard`, `step::run_all`); a project inserts its
  own steps into the list. The npm project the audit step covers is
  `Config::audit_npm_dir`.
- The library's subcommands moved into it as `cli::LibCommand` (behind its
  `clap` feature), dispatched by `cli::run`; a project's `xtask` flattens them
  into its own command enum and keeps only its own commands (`validate`,
  deploy, template tools, clean-cache). The thirteen wrapper modules
  under `xtask/src/` are gone.
- rustbase-xtask API, breaking against its 0.1.0 release (so its next release is
  0.2.0): `Config::dep_age_strict_env` is removed, since Dep-age no longer has a
  strict mode, and `DepAgeResult::warnings` is renamed `unchecked`, since its
  entries now fail the gate. A project that replaced the Dep-age step to make it
  strict, as the downstream project hoard did, can drop that replacement (#47).
- The frontend gates find the frontend from the project root, not the shell's
  working directory, so `cargo xtask` run from inside `frontend/` no longer
  reports "no frontend" and passes, and `validate`'s refusal of a frontend it
  would not check looks from the root too. A failing script is reported as
  every gate reports a failed run: the last 10 lines of the tool's stderr,
  else its stdout, of which `validate` shows the first 5 and the standalone
  command all 10 (the whole run is kept in a log: #55), as part of the
  error, so they also reach `target/validate.log`. A frontend without
  `node_modules` is told to run `npm install` inside it rather than `npm
  --prefix` (#44).
- The starter crates rustbase and rustbase-web carry publish = false, so a stray
  cargo publish cannot upload the template placeholders; a project that
  publishes its own crate removes it.
- A failing frontend gate names target/frontend-<gate>.log, which holds the
  whole run of its tool (both streams, rewritten each run), so the failing test
  names and counts the short report cuts are one file away. The log is for
  reading; the gates do not parse the tools' output (#55).
- A failing cargo xtask audit lists each vulnerability on a line of its own --
  the RUSTSEC id, crate, version, title and patched ranges; or the npm package,
  affected range, severity, GHSA advisory and available fix -- instead of only
  counting them. Advisory text comes from outside, so anything but printable
  ASCII is shown escaped (`\u{e9}`) (#62).
- The cooldown gate's waiver hint names the action instead of a commit: locally,
  commit the .dep-age-allow line and land it on the base branch before pushing;
  in CI, land it there and push again, since a re-run keeps the old base. It no
  longer names a branch (#62).
- rustbase-xtask API, breaking against its 0.2.4 release (so its next release is
  0.3.0): `cli::LibCommand::Fmt` and `cli::FrontendCommand::FrontendTest` became
  struct variants for `--check` and the filters, and `FrontendCommand` is now
  `#[non_exhaustive]`. A project that only flattens `LibCommand` into its own
  command enum changes nothing (#68).

### Fixed

- `cargo xtask backfeed-diff` now recognizes downstream feedback files that date
  `##` section headers (not only `###` per-entry headers), which previously made
  it report zero entries for such downstreams.
- xtask now compiles on Linux and macOS: clean_cache.rs had an import used only
  under cfg(windows) (an unused-import error under warnings = deny) and tripped
  unnecessary_wraps off Windows.
- cargo xtask validate derives its [N/M] step numbers from a single step table,
  so adding a gate can no longer print a wrong or duplicated step count.
- `cargo xtask backfeed-diff` no longer treats an indented `###` line in an
  entry's code sample as a new entry header, which silently truncated that
  entry.
- `cargo xtask sync-candidates` no longer offers the per-project `docs/todo.md`
  (or the new `fresh-reader-log.md`) as a sync candidate.
- xtask tests pass Rust 1.99 Clippy's `assert_is_empty` lint, which turned the
  gate red on the unpinned `stable` channel.
- `cargo xtask dep-age-check` fails on an unreadable `Cargo.lock` instead of
  passing with "no dependency changes" (#1).
- Frontend tooling clears all 9 npm audit advisories (5 high): `vitest` pinned
  to 4.1.11, transitive `brace-expansion`, `devalue`, `nanoid`, `postcss`,
  `postcss-selector-parser`, `source-map-js` and `undici` moved to patched
  versions past the 14-day cooldown (`postcss` and `undici` held by `overrides`
  until their newest releases age).
- Committed shell and PowerShell scripts with a shebang (`scripts/e2e.sh`,
  `scripts/kill-servers.sh`, `build.ps1`, the Stop hook) are now executable;
  `scripts/e2e.sh` previously failed with "permission denied" when run directly,
  as `/commit` does.
- `cargo xtask dep-age-check` treats a lockfile absent at the base commit as
  wholly new and checks every entry, instead of reporting "no baseline" (which
  failed the standalone command and CI when a lockfile was first added).
- `cargo xtask audit` and `cargo xtask deny` ran their tools in the shell's
  working directory, and audit looked for `frontend/` there too: run from inside
  `frontend/`, the `validate` Audit step skipped `npm audit` with only a
  warning. Both now run from the workspace root, and npm runs inside
  `frontend/`.
- `cargo xtask changelog add`, `cargo xtask todo` and `feedback-add` keep a
  file's line endings (a one-line edit no longer rewrites a CRLF file wholesale
  on Windows), and the first two wrap entry text without flattening lists or
  fenced code (from hoard). In `CHANGELOG.md`, sections are found by headings at
  the start of a line only, so nothing inside an indented entry -- a
  heading-like line, a code block, closed or not -- is mistaken for structure.
  In `docs/todo.md`, whose bodies sit at column 0, `todo add` refuses a body
  line that reads as a heading, and `add` / `done` refuse a file with a
  non-item heading after the first item.
  `feedback-add` removes an empty section's `_None yet._` placeholder when it
  adds the first entry (from bombyx).
- dep-preflight now runs `cargo update --precise` with the cargo of the
  toolchain in use (the `CARGO` variable), the same cargo as every other gate,
  instead of whichever `cargo` is first on `PATH`.
- Template frontend: vite 8.1.4 -> 8.1.6, so the `validate` Audit step passes
  again. It failed on three advisories against the launch-editor code vite
  bundles inside itself, which has no lockfile entry of its own, so only a vite
  upgrade fixes it: GHSA-587h-7wmj-h5pm (high), GHSA-698v-4r3x-3vcv and
  GHSA-r825-w8mm-5pxw (medium). The range in `frontend/package.json` moves
  from `^8.0.0` to `^8.1.6`, so an install without the lockfile cannot pick an
  affected 8.0.x or 8.1.x (#50).
- dep-preflight: when it cannot clear the tree (no aged version fits, or a
  changed crate could not be checked), its advice names the allow-list variable
  the project actually reads (`Config::dep_age_allow_env`, e.g.
  `HOARD_DEP_AGE_ALLOW`) instead of always `RUSTBASE_DEP_AGE_ALLOW`, which a
  project set up with `Config::for_project` ignores. It also gives the value
  format (`name@version[,...]`) and says `cargo xtask validate` reads it,
  not dep-preflight (#46).
- `cargo xtask clean-cache` counts the errors it hit by kind, as the
  downstream project hoard's copy did, e.g. `(3 PermissionDenied, 1 NotFound)`,
  before listing them, so a run of transient Windows locks reads differently
  from a real failure. One entry can give two errors (measuring it, then
  deleting it), so the counts are of errors, not entries (#45).
- cargo xtask deploy no longer nests the new frontend inside the existing
  frontend-dist. Its check for that directory ran without sudo and could not
  see into the mode-750 deploy directory, so a deploy left the site serving
  whatever was there before (on a freshly set-up host, the empty directory
  setup creates) and the next deploy failed (#10).
- `cargo xtask audit` no longer hangs on an advisory DB or npm registry that
  accepts the connection and never answers: each audit is stopped after three
  minutes (its whole process tree on Windows) and counts as a check that could
  not finish (a failure standalone, a warning inside `validate`).
  `RUSTBASE_AUDIT_TIMEOUT_SECS` (`<PROJECT>_AUDIT_TIMEOUT_SECS` in a derived
  project) sets a longer limit, for a machine's first run, which downloads the
  whole RustSec advisory database. npm's verdict takes the larger of
  its reported total and the sum of its severity counts, so a report with a 0
  or missing `total` beside non-zero counts fails, and a count that is not a
  whole number makes the report unreadable (#67).

### Removed

- The development diary (`docs/developer/DIARY.md`) and `/commit`'s step that
  appended to it; commit bodies and `CHANGELOG.md` carry that record.

## [0.17.0] - 2026-07-16

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
