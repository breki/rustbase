# Development Diary

This diary tracks functional changes to the codebase in
reverse chronological order.

---

### 2026-07-14

- Backfeed jutro Stage 4 (frontend): version header + Prettier/jscpd gates (v0.11.0)

    Tracker items 21, 20, 26. Extracted a shared
    `xtask/src/frontend.rs` (skip-vs-error classifier + npm
    runner) that `frontend-check`, the new `frontend-fmt`
    (Prettier; auto-fix default, `--check` read-only), and
    the new `frontend-dupes` (jscpd) all build on -- so the
    three frontend gates share one skip/error path. Both new
    gates wired into `validate` (now 8 steps). jscpd routes
    `.svelte` through its TypeScript tokenizer via
    `frontend/.jscpd.json` (verified: a duplicated
    component pair is flagged; without the mapping jscpd
    silently skips components). App header renders
    `__APP_VERSION__` (declared in a new
    `frontend/src/vite-env.d.ts` so svelte-check resolves
    it) -- which review caught breaking the vitest suite
    (undefined constant + an exact-text h1 assertion); fixed
    via a `define` in `vitest.config.js` and a relaxed
    assertion. That slipped past validate because the vitest
    suite was ungated, so also added a `frontend-test` gate
    (validate now 9 steps). Landmine caught mid-work: an
    `npm --prefix frontend
    install` run from the repo root re-anchored a stale
    `rustbase-e2e: file:..` dep and planted a
    `node_modules/rustbase-e2e -> <repo root>` junction;
    cleaned from all three npm files + node_modules (junction
    stripped before any delete). Strong motivation for the
    Stage 5 CLAUDE.md `file:`-dep warning.

- Backfeed jutro template feedback, Stage 3: e2e port isolation + multi-project .ports

    Tracker item 17 (no version bump -- test infra),
    expanded past jutro's original scope after the operator
    asked what happens when multiple rustbase-derived
    projects run at once. The Playwright harness stands up
    its own backend + Vite on isolated ports instead of
    sharing the dev server's with `reuseExistingServer:true`.
    `.ports` now defines all four ports
    (`backend_port`/`frontend_port` for dev,
    `e2e_backend_port`/`e2e_frontend_port` for the harness);
    `E2E_*` env vars override the e2e keys.
    `playwright.config.ts` is the source of truth for e2e
    ports (env -> .ports e2e_* -> 3001/5174), validates them,
    and pushes them to both webServers -- so a bare
    `npx playwright test` self-isolates identically to
    `e2e.sh`, closing the reviewer-found regression where a
    bare run would otherwise spawn on the dev ports.
    `reuseExistingServer:false`; Vite `strictPort` on
    isolated runs. `e2e.sh` resolves + validates (positive
    int, fail fast) the same ports and frees only those two
    via the extracted `scripts/lib/port-utils.sh` (also
    sourced by `kill-servers.sh`, with a numeric guard in
    `free_port`). Two projects coexist by each setting a
    distinct four-port block in its own `.ports`. Verified:
    `playwright test --list` + the smoke suite on both the
    `e2e.sh` and bare-`npx` paths.

- Backfeed jutro template feedback, Stage 2 (v0.10.4)

    xtask diagnostics + validate ergonomics (tracker items
    8, 10-13, 15-16). `validate` now runs cheap static gates
    (Fmt, Duplication, Clippy, Frontend) before the expensive
    dynamic ones (Test, Coverage), auto-fixes formatting by
    default (read-only behind `--check`), and prints an
    `iterate with: cargo xtask <gate>` hint on failure. Fixed
    three wrapper output gaps that hid where a failure was:
    `check` now matches short-format path-prefixed errors
    (it runs `--message-format=short`, but the extractor only
    matched long-format `error[` prefixes), `clippy` keeps
    the `-->` location line and surfaces bare `error:`
    denied-lint messages, and `test` compile errors include
    their location. The frontend gate now errors (not
    silently skips) when `frontend/` exists but isn't
    installed -- distinguished from a genuinely frontend-less
    project via a pure, unit-tested `classify` helper. Added
    a standalone `frontend-check` subcommand. Items 9 (e2e
    Windows build-lock) and 14 (dupes JSON aggregate)
    deferred -- see the plan doc for why.

### 2026-07-13

- Backfeed jutro Stage 1 item 7: ESLint TS-in-Svelte parser (v0.10.3)

    Frontend `npm run lint` was broken out of the box:
    `eslint.config.js` registered no TypeScript parser for
    `.svelte`, so the starter `App.svelte`'s `interface`
    declarations tripped a parsing error. Added
    `@typescript-eslint/parser` (`^8.0.0`) and
    `svelte-eslint-parser` (`^1.6.0`, already resolved
    transitively) plus flat-config `files` blocks for
    `**/*.ts` and `**/*.svelte` (the latter delegating the
    script body to the TS parser), and bumped `tsconfig`
    `lib` to ES2022. Held out of the v0.10.2 batch because
    it adds npm devDeps; landed after approval.

- Backfeed jutro template feedback, Stage 1 (v0.10.2)

    First batch from jutro's
    `docs/developer/template-feedback.md` (tracked in
    `docs/developer/jutro-improvements-plan.md`). Six
    diagnostic / safety fixes to tooling:
    `scripts/kill-servers.sh` now frees the dev ports
    (`:3000` / `:5173`) by *listener* instead of by
    process name, so it can't kill a production instance
    of the same binary on another port (full rewrite --
    this template had no port-scoped helpers). `xtask
    coverage` runs `cargo llvm-cov clean` before
    measuring, so stale `.profraw` no longer inflates the
    denominator and reads as a false regression, and it
    excludes `src/bin/` (multi-file binary shells) from
    the per-module floor. `xtask test <filter>` fails
    instead of printing `Test OK` when a filter matches
    zero tests. `scripts/e2e.sh` runs from the project
    root (cwd-independent) and defaults a quieter
    `RUST_LOG`. Item 7 (ESLint TS-in-Svelte parser) was
    held back -- it adds npm devDeps and needs the
    new-dependency approval.

### 2026-05-18

- Port hoard template feedback: junction guard, release-fast profile, dir_size warnings, Stop-hook stage labels (v0.10.1)

    Backfed eight items from hoard's
    `docs/developer/template-feedback.md`. Highest-
    impact fix: `xtask clean-cache` would follow
    Windows directory junctions (`mklink /J`) under
    `target/incremental/` and `remove_dir_all` the
    target tree -- now guarded via a new
    `is_reparse_or_symlink_meta` helper that checks
    `FILE_ATTRIBUTE_REPARSE_POINT` (pulled from
    `windows-sys`), applied in both `dir_size` (so the
    "freed" report no longer walks target trees) and
    `delete_entry`. `[profile.release]` reverted to
    cargo defaults so deployed binaries are fully
    optimised; the previous
    `incremental = true / codegen-units = 256`
    overrides moved into a new
    `[profile.release-fast]` for local iteration.
    `stop-check.sh` now runs stages via a `run_stage`
    helper that labels which one failed instead of
    masking earlier output via `&&` short-circuit.
    `dir_size` returns a structured
    `Vec<DirSizeWarning>` (was an inline-stderr
    `eprintln!` side-effect); `temp_scratch` dropped
    fragile `ThreadId` debug-string parsing.
    `/template-sync` now documents the Windows
    `git show <rev>:<path>` colon-mangling failure
    mode.

- Port rustwerk template feedback + add /template-backfeed + Stop-hook fmt-check (v0.10.0)

    Rolls up the multi-commit session that ported
    rustwerk's template-feedback items, hardened
    `/template-sync`, fixed the `xtask check`
    "aborting" filter and added a stderr fallback for
    non-rustc failures, shipped a workspace
    `clippy.toml` doc-valid-idents allowlist, added
    the new `/template-backfeed` slash command
    (inverse of `/template-sync`: pulls
    downstream-logged template improvements back into
    this template repo), and expanded the Stop hook
    to include `cargo fmt --check` so fmt drift can't
    bypass `/commit`'s validate gate on `chore:`
    commits.

    Release-bookkeeping cleanup: the CHANGELOG had
    been treating `[Unreleased]` as a rolling
    accumulator since v0.5.0, so v0.6.0, v0.7.0,
    v0.8.0, and v0.9.0 were all bumped in
    `Cargo.toml` without ever cutting their CHANGELOG
    sections. This release retroactively splits the
    accumulated block into proper `[0.6.0]`,
    `[0.7.0]`, `[0.8.0]`, `[0.9.0]`, and `[0.10.0]`
    headings, attributing each bullet to its real
    release using DIARY entries and the git log
    between version tags as ground truth.

### 2026-05-17

- Coverage failures now include uncovered-line ranges (v0.8.0)

    Implements item 7 of the ledgerstone improvements
    plan. When `cargo xtask validate`'s Coverage step
    fails because a module dropped below
    `MODULE_THRESHOLD` (85%), the error message now
    includes the actual uncovered line ranges per
    failing module:

    ```
    modules below coverage threshold:
        api/routes.rs: 72.5%
          uncovered: 84-93, 209-221
        api/dto.rs: 60.0%
          uncovered: 12, 45-60
    ```

    Saves the round trip of running a separate detail
    command at the exact moment a coverage failure
    happens. Implementation drops `--summary-only` from
    the existing `cargo llvm-cov --json` call so the
    per-file `segments` array survives into the output,
    then parses segments into a typed `Segment` struct
    (custom `Deserialize` from a 6-element JSON tuple,
    so older `llvm-tools-preview` versions that emit
    5-element segments surface as a hard parse error
    rather than silently misclassifying gap regions).
    `uncovered_ranges` walks the segment list,
    explicitly handles the trailing segment (windowed
    iteration would otherwise drop end-of-file
    uncovered spans), and merges adjacent ranges.

    Public API: `coverage::CoverageFailure` enum
    (`Overall { pct, threshold }` /
    `Modules(Vec<FailingModule>)`) and
    `coverage::format_failure(&CoverageFailure) ->
    String` separate the structured data from the
    rendering so future consumers (CI annotations,
    JSON export) can introspect failures directly.
    `coverage::THRESHOLD` renamed to
    `OVERALL_THRESHOLD` for symmetry with the existing
    `MODULE_THRESHOLD`.

- Apply ledgerstone template improvements (Batch B, clean-cache) (v0.7.0)

    Ported Ledgerstone's `cargo xtask clean-cache`
    command. Empties `target/{debug,release}/
    incremental/` (preserves the directories themselves
    so cargo refills them on next build), reports bytes
    freed per directory and a total. Manual invocation
    only -- never auto-wired into builds or deploys
    since auto-cleanup defeats incremental caching. The
    original motivation was Ledgerstone's 32.8 GB of
    stale `incremental/` content accumulating across
    months of cargo invocations on Windows.

    The port hardened two aspects beyond the
    Ledgerstone original:

    1. **Symlink/junction safety.** The original used
       `path.is_dir()` + `fs::remove_dir_all`, both of
       which follow symlinks. A directory junction
       under `incremental/` (more common on Windows
       than people realise) could redirect deletion
       outside the workspace. The port uses
       `DirEntry::file_type()` (no traversal) and
       routes symlinks through `remove_file` /
       `remove_dir` so the link itself unlinks but the
       target survives. Regression test plants a
       symlink-to-outside and asserts the target's
       contents remain.

    2. **Continue-on-error.** The original `?`-aborted
       on the first failed deletion. Since the tool
       exists specifically to clean caches that
       Windows AV and rust-analyzer transiently lock,
       aborting on a single locked file defeated the
       purpose. `clear_dir_contents` now returns
       `(bytes_freed, Vec<String> errors)`,
       accumulates per-entry failures, and continues.
       `clean_cache` prints the error list after the
       totals and returns a final `Err` with the
       count.

    Shared utilities (`dir_size`, `fmt_bytes`,
    `temp_scratch`) live in `helpers.rs` so future
    "disk-usage" or "clean" commands can reuse them
    instead of copy-paste.

- Apply ledgerstone template improvements (Batch A) (v0.6.0)

    Three suggestions from
    `docs/developer/ledgerstone-improvements-plan.md`
    Batch A landed; two were skipped as not applicable.

    `Cargo.toml` gains a `[profile.release]` block with
    `incremental = true, codegen-units = 256`. The
    comment block documents the non-default nature and
    spells out when derived projects should override
    (CPU-bound services, multi-user binaries, `cargo
    install` targets). Empirical data is attributed to
    Ledgerstone rather than asserted as universal.

    `frontend/package.json` gains `npm run fix`
    (prettier + eslint --fix) and `npm run check:all`
    (check, lint, format:check, test, build) as
    discoverable aggregator scripts.

    `cargo xtask validate`'s Test step now runs
    `cargo test -p xtask` only, since the Coverage step
    runs every non-xtask test under llvm-cov
    instrumentation. Refactor introduces a `Scope`
    enum so `test()` and `test_check_xtask()` share
    `build_args`, and a `report_failure` helper so both
    produce identical rich diagnostics. Step label
    renamed to `Test (xtask only)` to avoid misleading
    "Test OK" output. Validate wall time drops to ~10s
    on a warm tree.

    Skipped: deploy-prod `Tee-Object` (deploy is xtask
    now, streams natively) and jscpd stale-dir cleanup
    (rustbase uses `code-dupes`, not jscpd).

- Add deploy-as-xtask + sort helper from hoard (v0.5.0)

    Ported hoard's deploy/deploy-setup `xtask`
    subcommands into the template, generalized for any
    Linux+systemd target. Four new xtask modules
    (`deploy`, `deploy_config`, `deploy_remote`,
    `deploy_setup`), a sandboxed `rustbase-web.service`
    unit, `.deploy.sample` config template, and
    `docs/deployment.md`. Wrapped by `build.ps1 deploy`
    and `build.ps1 deploy-setup`.

    The `deploy_config` loader validates `rpi_host`,
    `rpi_user`, and `deploy_path` against a strict
    allowlist before any value reaches an SSH command
    string. `REQUIRED_DEPLOY_PATH = "/opt/rustbase"` is
    threaded into the remote bash tripwire as `$2`, so
    the literal and the constant cannot drift.

    Deploy is restructured into "stage everything, then
    stop / swap / start": frontend dist is scp'd to a
    remote staging dir before the service is stopped, and
    any failure during the swap window now attempts
    `systemctl start` as a rollback rather than leaving
    the service down. Frontend install is a real
    POSIX-atomic rename swap (the path is never absent).

    Service unit binds `127.0.0.1` by default (paired
    with a header note on reverse-proxy / TLS),
    `MemoryHigh=256M` + `MemoryMax=1G` instead of a
    single hard 256M cap that would OOM-kill under
    modest load.

    Added `frontend/src/lib/sort.ts` with
    `Intl.Collator`-backed `compareNames` / `compareIds`
    (case-insensitive; `compareIds` is numeric-aware so
    `CAB-2` precedes `CAB-10`). Accepts an optional
    `locale` argument; default uses runtime locale.

### 2026-04-16

- Add `build.ps1 dev` command + frontend TypeScript (v0.4.0)

    Added `Invoke-Dev` to `build.ps1`: parses `.ports` for
    `backend_port`, pre-builds the backend so compile errors
    surface immediately, launches the compiled
    `rustbase-web.exe` directly (not via `cargo run`) so
    Ctrl+C cleanup via `Get-CimInstance` descendant
    enumeration actually kills the web server instead of
    orphaning it. Guards against missing
    `frontend/node_modules` before starting the backend.

    Added frontend TypeScript: `typescript`,
    `@tsconfig/svelte`, `svelte-check` dev deps;
    `tsconfig.json` extending `@tsconfig/svelte` (only
    `noEmit` override); renamed `main.js` to `main.ts` with
    a `getElementById` null guard; converted `App.svelte`
    to `<script lang="ts">` with `StatusResponse` /
    `GreetingResponse` interfaces and `res.ok` + partial
    narrowing on `fetch` results. Added `npm run check`
    script. Wired `svelte-check` into `cargo xtask validate`
    as step 6 via new `xtask/src/frontend_check.rs` module
    (skips gracefully when no frontend or `node_modules`).

    Fixed pre-existing bad pin: `@eslint/js` was `^10.2.0`
    but the latest published version on npm is `10.0.1`,
    so `npm install` failed with `ETARGET` on clean
    clones. Relaxed `typescript` pin from `^5.8.0` to
    `^5.0.0` to match peer requirements. Added `/health`
    to `vite.config.js` proxy list so dev mode matches
    production topology and the `health endpoint returns
    OK` E2E test actually exercises the backend.

### 2026-04-15

- Apply 22 template improvements from hoard (v0.3.0)

    Modularized `xtask` into 8 modules with agent-friendly
    stepwise `[1/N]` output format. Added `cargo xtask check`
    fast compile check. Improved `/commit` skill: code
    reviews before E2E, expanded review scope to frontend
    and deployment files, Deployment category in Red Team,
    all findings reported via `AskUserQuestion`. Added
    `/check`, `/test`, `/validate` slash commands. Updated
    `/todo` to support adding items with arguments.
    Converted Playwright config and E2E tests to TypeScript.
    Fixed `127.0.0.1` to `localhost` and `cd frontend` to
    `cwd` option in Playwright config. Created cross-platform
    `kill-servers.sh` and `e2e.sh` scripts. Added
    `docs/ai-agents/guidelines.md`. Added E2E test policy
    to `CLAUDE.md`. Fixed `@eslint/js` version alignment
    and `vitest` `passWithNoTests`.

### 2026-04-10

- Resolve open review findings (v0.2.1)

    Moved inline `${{ }}` to `env:` blocks in release
    workflow (RT-009). Release now fails on empty notes
    (RT-010). Checksums use `nullglob` + explicit globs
    (RT-011, RT-012). Awk uses exact pattern match for
    version extraction (RT-013). `create_router` accepts
    `&Path` (AQ-006). `SocketAddr` constructed via
    `IpAddr` + `SocketAddr::new` (AQ-007). Added
    `edition = "2024"` to `rustfmt.toml`. Documented
    `code-dupes` prerequisite in README.

- Add `/template-sync` command (v0.2.0)

    New slash command for syncing derived projects with
    upstream template changes. Added `.template-sync.toml`
    to track template origin (commit SHA + version).
    The command fetches upstream, categorizes changes,
    and helps selectively apply updates while preserving
    project customizations.

- Address template feedback: 8 fixes (v0.1.2)

    Replaced `__dirname` with `import.meta.dirname` in
    `vite.config.js`. Narrowed `tokio` features from
    `full` to explicit list. Changed `/health` endpoint
    to return JSON. Replaced `7z` with
    `Compress-Archive` in release workflow and added
    empty release notes warning. Added ESLint + Prettier
    for frontend linting. Added Vitest + Testing Library
    for frontend unit tests. Documented double-compile
    in `build.ps1`.

- Address template feedback from hoard project (v0.1.1)

    Coverage now excludes `xtask` crate and binary
    `main.rs` entry points via `--exclude` and
    `--ignore-filename-regex`. Added `cargo xtask dupes`
    for code duplication detection (6% threshold,
    `code-dupes`). Added `/template-improve` command,
    TDD guidance, and enforced `/commit` for all commits.
