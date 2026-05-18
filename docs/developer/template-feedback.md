# Template feedback

Issues, improvements, and observations about the
[rustbase](https://github.com/breki/rustbase) template.

This file uses three lifecycle sections, the same shape
adopted by Ledgerstone (a downstream project) and now
shipped with the template itself:

- **Open divergences** -- things the project knows are
  suboptimal, missing, or differently-shaped than the
  ideal template baseline. In a derived project these
  are intentional or pending differences from the
  template; in this template repo they are known
  template issues awaiting fix.
- **Resolved** -- entries closed out by a retrofit /
  fix commit. Keeps the history visible without
  cluttering the open list.
- **Suggestions to flow back to the template** -- in a
  derived project, this is where ideas live that the
  project wants to push upstream. In this template repo
  the section is informational (there is no upstream),
  but the structure is preserved so new entries route
  identically across template and derived projects.

`/template-improve` adds new entries by asking which
section they belong to.

---

## Open divergences

### `build.ps1 build` command double-validates

`Invoke-Build` (line 36) calls `Invoke-Validate`, which
runs `cargo xtask validate` (fmt + clippy + test +
coverage). It then calls `Invoke-BuildOnly`, which runs
`cargo build --release`. But `cargo xtask validate`
already compiled the entire workspace (in debug mode)
as part of running clippy and tests.

The result: a full `.\build.ps1 build` compiles the
workspace twice -- once in debug for validate, once in
release for the final binary. This is expected if the
intent is "check everything, then produce a release
binary." But if a user just wants a validated release
build, there is no single command that avoids the
double-compile.

**Suggestion:** Document that `build` intentionally
compiles twice (validate in debug, then release), so
users understand the cost. Alternatively, consider a
`build-release` command that runs clippy and tests
against the release profile directly, avoiding the
redundant debug build.

### Playwright fixture-isolation pattern not shipped

`rustbase-web` has no persistent data -- `/api/status`
and `/api/greeting` return static values -- so a
fixture-isolation example doesn't fit the current
sample app. The pattern becomes relevant the moment a
downstream project adds a data store.

**Suggestion (for when a data-persistence example is
eventually added to the template):** include the
fixture-isolation pattern alongside it (globalSetup
copies `e2e/fixtures/*.json` to `test-data/`, backend
run with `--data test-data`). Until then, leave it to
individual projects.

---

## Resolved

### 2026-05-18 -- `clean_cache` Windows junctions bypassed the symlink guard

Surfaced from hoard's template feedback (2026-05-18).
`delete_entry` in `xtask/src/clean_cache.rs` dispatched
on `FileType::is_symlink()`, but on Windows that flag
is only set for `IO_REPARSE_TAG_SYMLINK` -- directory
junctions (`mklink /J`) returned `false` and fell into
the `is_dir()` branch, where `remove_dir_all` would
traverse the junction and delete the *target* tree
outside the workspace. **Fix:** added an
`is_reparse_or_symlink` helper that checks
`FILE_ATTRIBUTE_REPARSE_POINT` via
`MetadataExt::file_attributes` on Windows; reparse
points unlink via `remove_dir` with a `remove_file`
fallback. Added a `cmd /c mklink /J`-based regression
test (no elevation required, unlike `symlink_dir`).

### 2026-05-18 -- `[profile.release]` overrides shipped slow deployed binaries

Surfaced from hoard's template feedback (2026-05-18).
Root `Cargo.toml` set `[profile.release] incremental =
true, codegen-units = 256`. The accompanying doc
correctly named "any project shipping a binary to
multiple users" as a case to skip -- but every derived
project's `cargo build --release` (including deploy
flows) silently inherited a measurably slower binary
for no iteration-speed gain. **Fix:** moved the
overrides into a new `[profile.release-fast]` that
inherits from `release`; `[profile.release]` stays at
cargo defaults so deployed binaries are fully
optimised. Local fast iteration uses
`cargo build --profile release-fast`.

### 2026-05-18 -- `stop-check.sh` `&&` chain dropped earlier-stage output

Surfaced from hoard's template feedback (2026-05-18).
The hook ran `output=$(cargo fmt --check && cargo
xtask clippy && cargo xtask test)`. On multi-stage
failure the captured `$output` held only the
short-circuited stage's output with no label, so when
Claude fixed clippy the previously-masked test
failure resurfaced looking "new" -- exactly the
fix-loop amplification the `stop_hook_active` guard
is meant to prevent. **Fix:** extracted a `run_stage
"label" cmd...` helper that runs stages sequentially
and prefixes failing output with `=== <stage> failed
===`, so Claude sees both the stage label and the
full failure body on the first hook run.

### 2026-05-18 -- `dir_size` was a library helper that printed to stderr and undercounted silently

Surfaced from hoard's template feedback (2026-05-18).
`dir_size` in `xtask/src/helpers.rs` returned
`Ok(total)` but called `eprintln!("warning: ...
(skipping)")` from inside the recursive walk on
per-entry failures, contradicting its
`Result<u64, String>` contract ("size or error") with
"best-effort size plus stderr side-effects." **Fix:**
changed the signature to
`Result<(u64, Vec<String>), String>` so per-entry
failures populate a warnings vector and callers
decide what to do. `clean_cache::clear_dir_contents`
folds those warnings into its own per-entry error
list, so warnings now surface alongside deletion
failures in the final report instead of being dumped
to stderr mid-loop.

### 2026-05-18 -- `temp_scratch` used fragile `ThreadId` debug parsing and bare `unwrap`

Surfaced from hoard's template feedback (2026-05-18).
`temp_scratch` in `xtask/src/helpers.rs` formatted
`thread::current().id()` via `{:?}` and filter-to-
digits to build a unique suffix. `ThreadId`'s `Debug`
output is unspecified by std and could change across
Rust versions, and the process-wide `AtomicU64`
counter is already unique across parallel test
threads, so the thread-id component was redundant.
The trailing `fs::create_dir_all(&dir).unwrap()` also
gave a generic panic that misled CI diagnostics.
**Fix:** dropped the `ThreadId` parsing (kept
`pid + AtomicU64`) and replaced `unwrap` with
`unwrap_or_else` carrying a descriptive panic that
names the failing path.

### 2026-05-18 -- `/template-sync` didn't document the Windows `git show <rev>:<path>` failure

Surfaced from hoard's template feedback (2026-05-18).
On Windows shells the `:` separator in `git show
template/main:crates/rustbase/Cargo.toml` (and
similar `revspec:path` forms) can be mangled into a
`;` separator, producing an "ambiguous argument"
error with no obvious workaround. **Fix:** added a
"Windows note" to step 9 of
`.claude/commands/template-sync.md` prescribing the
cross-platform `git show <rev> -- <path>` /
`git diff <rev> -- <path>` form, which keeps the
path as a separate argument and sidesteps the colon
mangling. The note also covers any step-7 site that
reads upstream files via `revspec:path`.

### 2026-05-17 -- `xtask check` "aborting" filter dropped legitimate user errors

Surfaced from rustwerk's template feedback (2026-04-19,
fixed there locally). `extract_error_lines` in
`xtask/src/check.rs` used `!l.contains("aborting")` to
strip rustc's `error: aborting due to N previous errors`
summary line. Any legitimate user error whose message
contained the substring "aborting" (e.g.
`compile_error!("aborting: feature X required")`) was
also silently filtered out, producing `FAILED: 0
compilation error(s)` with no body. **Fix:** anchored
the filter to the exact summary prefix
(`!l.starts_with("error: aborting due to")`) and added
a regression test (`keeps_user_errors_mentioning_aborting`).

### 2026-05-17 -- `xtask check` reported "0 compilation error(s)" for non-rustc failures

Surfaced from rustwerk's template feedback (2026-04-19).
When cargo exited non-zero for reasons other than
compile errors (manifest parse failure, corrupted
`Cargo.lock`, missing registry network, unsupported
flag on older cargo), the first diagnostic line did
not match the `error[`/`error:` prefix filter, so the
output became `FAILED: 0 compilation error(s)` with
nothing to diagnose. **Fix:** when `errors.is_empty()`
but the process exited non-zero, `check()` now prints
the last ~20 non-empty stderr lines verbatim under a
`FAILED: cargo exited non-zero with no matched error
lines` banner so the user sees an actionable signal.

### 2026-05-17 -- `clippy::pedantic` `doc_markdown` flagged common infra identifiers

Surfaced from rustwerk's template feedback (2026-04-21).
Writing doc comments that mention `PowerShell`, `FFI`,
`JSON`, `WebSocket`, `macOS`, `GitHub` and similar
well-known terms required each occurrence to be
backticked under `clippy::doc_markdown`, producing
prose with unnecessary backticks for the reader.
**Fix:** added a workspace-root `clippy.toml` with a
curated `doc-valid-idents` allowlist that extends
clippy's defaults via the `".."` sentinel. Documented
in CLAUDE.md under "Lints: `doc_markdown` allowlist
via `clippy.toml`" with a note that derived projects
should append, not redefine.

### 2026-05-17 -- `/template-sync` granted `Bash(git checkout:*)` it never used

Surfaced from rustwerk's template feedback (2026-04-19).
The slash command's `allowed-tools` front-matter
included `Bash(git checkout:*)`, but the documented
workflow only used `git diff`, `show`, `log`,
`rev-parse`, `fetch`, and `status` for reading, plus
`Edit` / `Write` for applying. The glob permitted
destructive variants (`checkout -f <ref>`,
`checkout -- .`, `checkout -- <path>`) that can
destroy uncommitted work or move HEAD onto an
untrusted template ref whose `.gitattributes` /
hooks could activate on the next git command --
combined with diff content being LLM input, that's
an escape vector. **Fix:** removed
`Bash(git checkout:*)` from the allowed-tools list
in `.claude/commands/template-sync.md`.

### 2026-05-17 -- `/template-sync` trusted upstream URL from `.template-sync.toml`

Surfaced from rustwerk's template feedback (2026-04-19).
Step 3 of the workflow read `repo` from
`.template-sync.toml` and fed it to
`git remote add template <url>`. A malicious PR
changing that field (or a typo-squat lookalike)
would silently redirect future syncs; `git remote
add` also historically accepted hostile URL forms
(`ext::sh`, `--upload-pack=...`). **Fix:**
hard-coded the expected upstream
(`https://github.com/breki/rustbase`) in the slash
command. The command now asserts the
`.template-sync.toml` value matches exactly before
proceeding, refuses anything outside the
`https://github.com/breki/` prefix, and verifies any
pre-existing `template` remote URL also matches.
Bootstrap flow updated to use the same hard-coded
URL.

### 2026-05-17 -- `/template-sync` "all" option bypassed per-file review

Surfaced from rustwerk's template feedback (2026-04-19).
Step 6 accepted "all" to apply every recommended
change in bulk. Because the agent reads raw upstream
commit messages and diff bodies during step 5
categorization, a single compromised upstream commit
could smuggle an instruction that bulk-apply would
honour. **Fix:** removed "all" as an accepted answer.
Users must opt in by category name or specific file
path; "none" still works as a no-op marker update.

### 2026-05-17 -- No pwsh-twin convention for `.sh` scripts

Surfaced from rustwerk's template feedback (2026-04-21).
The template targeted Windows as a first-class
platform but had no convention for how `scripts/*.sh`
should reach Windows users -- ad-hoc `.ps1` ports
risked duplicating bash logic that then drifted on
bugfixes. **Fix:** added a "Shell wrappers" section
to CLAUDE.md prescribing "non-trivial logic lives in
`cargo xtask`; shell files are thin wrappers only,"
with canonical bash and pwsh wrapper shapes and an
explicit carve-out for cases that can't live in Rust
(process cleanup, pre-cargo bootstrap).

### 2026-05-17 -- CLI-only downstream had no documented "safe to delete" list

Surfaced from rustwerk's template feedback (2026-04-19,
`N/A for rustwerk` note). The template's full-stack
shape meant CLI-only downstreams had to discover by
trial which directories were safe to remove. **Fix:**
CLAUDE.md's "Workspace Crates" section now lists the
full set safe to delete for a CLI-only project
(`crates/rustbase-web/`, `frontend/`, `e2e/`,
`.mise.toml`, `playwright.config.*`, `.ports*`,
`scripts/e2e.sh`) along with the corresponding
`Cargo.toml`, `xtask validate`, and `build.ps1`
pruning steps, and notes that `/template-sync` will
auto-skip those paths once they no longer exist.

### 2026-05-17 -- `.gitignore` only hid root-anchored `/target/`

Surfaced from trmnl-bellwether's template feedback
(2026-04-19). The template's `.gitignore` had
`/target/`, so any nested `target/` (e.g.
`crates/<name>/target/` created when cargo was invoked
from inside a crate directory) showed up as untracked
and was easy to accidentally `git add .` into a commit.
**Fix:** replaced `/target/` with `target/` plus
`**/target/` for belt-and-braces, so any `target/`
directory at any depth is ignored.

### 2026-05-17 -- `cargo xtask test` had no `--ignored` forwarding

Surfaced from trmnl-bellwether's template feedback
(2026-04-19). CLAUDE.md forbids raw `cargo test`, but
the `xtask test` wrapper had no way to run
`#[ignore]`-tagged tests (the standard Rust idiom for
"manual tool" tests that shouldn't run in
`validate`), so projects using `#[ignore]` for manual
tools hit a dead end. **Fix:** added a `--ignored`
flag to `XCommand::Test`, introduced
`TestOptions<'a> { filter, verbose, ignored }` so the
signature stays readable as flags accrete, threaded
`--ignored` through `build_args` after an explicit
`--` separator, and added two `build_args` unit tests
covering ignored-alone and ignored-plus-filter.
CLAUDE.md's Build Commands table updated to advertise
`cargo xtask test --ignored`.

### 2026-05-17 -- `/commit` skill ambiguous about how to hand the diff to review subagents

Surfaced from trmnl-bellwether's template feedback
(2026-04-17). The previous wording ("Pass the full
`git diff` output to both agents and tell them to
read the relevant source files") didn't prescribe a
mechanism, so models reflexively reached for
`tokio::fs::write("/tmp/foo-diff.txt", ...)` or
`git diff --cached > /tmp/foo-diff.txt`. On Windows
with Git Bash, `/tmp` maps outside the workspace,
isn't git-ignored, and is invisible to the user.
**Fix:** the skill now explicitly tells each subagent
to run `git diff --cached` itself as its first step
(both agents have Bash), forbids `/tmp` paths, and if
a file is genuinely needed prescribes a
git-ignored workspace-local path under `target/`. The
mechanism is also reusable: any future
"capture-output-and-hand-to-subagent" pattern in the
skill should prefer subagent-runs-the-command or
`target/`-local files.

### 2026-05-17 -- `/commit` resolved-log entry format was unspecified

Surfaced from trmnl-bellwether's template feedback
(2026-04-17). The skill said "remove from the open
log and insert at the top of the resolved log with
the fix date and resolution" but was ambiguous about
whether to preserve the original Description / Impact
/ Suggested-fix body or replace it with a terse
resolution-only entry. Different agents picked
different formats across sessions, causing cross-PR
inconsistency in the same project's resolved log.
**Fix:** the skill now prescribes a terse format
(`### <ID> -- <title>` heading + `**Category:**` line
+ `**Resolution:**` line with the fix date and a 1-3
sentence note) and explicitly says not to preserve
the original body in the resolved entry.

### 2026-05-17 -- `/commit` CHANGELOG rule keyed on commit type, not observable effect

Surfaced from trmnl-bellwether's template feedback
(2026-04-17). The skill said "Skip [CHANGELOG] for:
chore, ci, style, docs-only changes." But chores
routinely contain user-visible behaviour changes
(e.g., a default port change committed as `chore:`)
where a CHANGELOG entry is genuinely needed. **Fix:**
the rule now keys on user-observable effect, not
commit type: add a `[Unreleased]` entry whenever a
user of the software would see a difference (new
feature, fixed bug, changed default, removed flag,
new config knob, port change, new env var, ...) --
even if the commit type is `chore`. Skip only for
commits with no user-observable effect (pure
refactors, internal tooling, test-only changes,
docs-only edits, CI/lint tweaks invisible to users).

### 2026-05-17 -- Stop hook ran full `cargo xtask validate`, including ~15s coverage step

Surfaced from kozmotic's template feedback (2026-05-04).
The Stop hook ran `cargo xtask validate` -- fmt +
clippy + tests + coverage + duplication + frontend
check -- on every stop where Rust files had changed.
Coverage alone adds ~15s on a small codebase, and the
Stop hook fires often enough during interactive work
that the cost compounded. **Fix:** Stop hook now runs
`cargo xtask clippy && cargo xtask test` only,
skipping coverage, duplication, fmt-check, and the
frontend type check. Full validate still runs from
`/commit` (and is available manually) so coverage and
duplication signal is preserved at the commit gate;
the Stop hook is positioned purely as a fast
interactive safety net for lint + test regressions.

### 2026-05-17 -- `scripts/` shipped 5 trivial one-line cargo wrappers redundant with xtask

Surfaced from kozmotic's template feedback (2026-05-04).
The template shipped `scripts/build.sh`, `clippy.sh`,
`test.sh`, `fmt.sh`, `validate.sh` -- each a one-line
`cargo ...` wrapper. CLAUDE.md already steers users to
`cargo xtask` ("Never use raw `cargo test`"), so the
shell wrappers added discoverability noise without
adding capability. **Fix:** deleted the 5 trivial
wrappers. Kept `scripts/e2e.sh` and
`scripts/kill-servers.sh` -- both contain non-trivial
process-cleanup logic (PowerShell `Get-CimInstance`
filtering, pkill patterns) that doesn't fit naturally
as an xtask subcommand.

### 2026-05-17 -- No documented pattern for hardware-bound code under the 90% coverage gate

Surfaced from kozmotic's template feedback (2026-05-04).
The 90% coverage gate assumes everything is testable,
but real CLI projects have I/O paths (audio, network,
native APIs) that can't run in CI. Kozmotic's working
recipe -- extract the hardware-bound call into a
sibling submodule, add the submodule to the coverage
`IGNORE_REGEX`, and add a `*_TEST_*` env-var escape
hatch so the parent module's success/error branches
remain in the gate -- was undocumented. **Fix:** added
a "Coverage exceptions for hardware-bound code" section
to `CLAUDE.md` next to the existing workspace-lints
override recipe, describing the three-step pattern and
the explicit "use a fake trait if you can" caveat.

### 2026-04-16 -- `@eslint/js` pin in `frontend/package.json` was unresolvable

The template shipped `"@eslint/js": "^10.2.0"`, but
10.0.1 is the latest published version on npm, so
`npm install` from a clean clone failed with
`ETARGET`. The lockfile had 10.0.1 installed, masking
the problem until someone ran `install` without the
lock. **Fix:** changed the pin to `^10.0.0`. Template
should pin to a version that actually exists and
ideally be kept in sync by a bot like Renovate.

### 2026-04-16 -- Deploy logic belongs in xtask, not bash scripts

Initially deferred for a dedicated design discussion
because hoard's deploy was specific to a Raspberry Pi
/ SSH / systemd workflow. Resolved in v0.5.0 by
porting hoard's deploy-as-xtask pattern (generalized
for any Linux+systemd target) into the template.

### `vite.config.js` uses CommonJS `__dirname` in ESM context

`frontend/package.json` declares `"type": "module"`,
which means all `.js` files are ES modules. However,
`vite.config.js` used `__dirname` (lines 9, 22), which
is a CommonJS-only global. This worked only because
Vite's config loader shims `__dirname`, but it is not
idiomatic and would break if copy-pasted elsewhere.

**Fix:** Replaced with `import.meta.dirname`
(available since Node 21.2 / 22+).

### Tokio used `features = ["full"]` unnecessarily

`crates/rustbase-web/Cargo.toml` depended on
`tokio = { version = "1", features = ["full"] }`. The
`full` feature flag pulled in every Tokio subsystem
including unused ones, increasing compile time and
binary size.

**Fix:** Replaced with explicit feature list
(`["macros", "rt-multi-thread", "net", "signal"]`).

### `/health` endpoint returned plain text, not JSON

The `/health` endpoint returned `"OK"` as plain text,
while `/api/status` and `/api/greeting` both returned
JSON. Inconsistent for API consumers and orchestrators
that expect JSON.

**Fix:** `/health` now returns `{ "status": "ok" }`
with `Content-Type: application/json`.

### Release workflow assumed `7z` is available on Windows

`.github/workflows/release.yml` used `7z a "${STAGING}.zip" ...`.
GitHub-hosted Windows runners include 7-Zip, but
self-hosted runners may not.

**Fix:** Replaced with PowerShell's built-in
`Compress-Archive`, requiring no external tools.

### Release notes extraction from CHANGELOG was fragile

The release workflow used an `awk` script to extract
notes by heading match. Format deviations would
silently produce an empty `release_notes.md`.

**Fix:** Release workflow now warns (and later fails)
when CHANGELOG extraction produces empty release notes;
extraction uses exact version match instead of
substring.

### No frontend linting or formatting tools

The Rust side had strict quality gates; the frontend
had nothing comparable. No ESLint config, no Prettier
config; `cargo xtask validate` did not check frontend
code quality.

**Fix:** Added ESLint with `eslint-plugin-svelte` and
Prettier with `prettier-plugin-svelte`. `lint`,
`format`, `format:check` scripts in
`frontend/package.json`. `svelte-check` step added to
`cargo xtask validate`.

### No frontend unit test infrastructure

The template included Playwright for E2E but had no
setup for unit/component tests in isolation.

**Fix:** Added Vitest with `@testing-library/svelte`
and `jsdom`; `test` script wired into the frontend.

---

## Suggestions to flow back to the template

(In a derived project, this section lists ideas the
project wants to push upstream into the template. In
this template repo itself there is no upstream, so the
section is normally empty. It is preserved so new
entries route identically across template and derived
projects -- and so the file's section shape ships as
part of the template.)
