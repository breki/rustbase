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
