# Development Diary

This diary tracks functional changes to the codebase in
reverse chronological order.

---

### 2026-05-17

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
