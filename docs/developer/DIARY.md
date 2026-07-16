# Development Diary

This diary tracks functional changes to the codebase in
reverse chronological order.

---

### 2026-07-16

- Template tooling: determinism moved into `cargo xtask`

    Made `/template-backfeed`, `/template-improve`, and
    `/template-sync` scalable by moving delta determination and
    log bookkeeping out of the LLM into deterministic,
    unit-tested `cargo xtask` commands; the LLM keeps only the
    judgment. Four new subcommands: `backfeed-diff` (downstream
    feedback entries on/after a ledger watermark) and
    `backfeed-record` (advance the watermark) in
    `xtask/src/backfeed.rs`; `feedback-add` (append a
    `template-feedback.md` entry with a minted
    `tf-<date>-<slug>` ID, section-top insert, dedup) in
    `xtask/src/feedback.rs`; `sync-candidates` (categorized
    `git diff --name-status` delta minus a never-sync
    bookkeeping set) in `xtask/src/sync.rs`. New machine-owned
    `docs/developer/backfeed-ledger.toml` (hand-parsed, no
    `toml` dep) seeded with jutro=2026-07-14,
    clockdump=2026-07-15 so their history is never re-scanned.
    Shared `today_iso` / `is_iso_date` / `extract_iso_date`
    date helpers added to `helpers.rs`. Rewrote the three
    slash commands to call the new commands, and recorded the
    determinism-vs-judgment principle in `CLAUDE.md`. Full
    `cargo xtask validate` green (11/11). Implements
    `template-tooling-cli-redesign`.

- Coverage ignores configurable via workspace metadata

    `cargo xtask coverage` now merges
    `[workspace.metadata.coverage] ignore = [...]` from the
    root `Cargo.toml` into its `--ignore-filename-regex`
    baseline, so a derived project excludes a hardware-bound
    leaf module (see the "Coverage exceptions" recipe) via
    manifest config instead of forking `coverage.rs`. Baseline
    unchanged when the section is absent; a missing/unreadable
    manifest degrades to the baseline rather than failing the
    gate. Hand-parsed (no `toml` dep), matching the
    `gate.rs`/`deploy_guard.rs` convention. Verified
    end-to-end: a temporary `ignore = ["rustbase-web"]` dropped
    the measured line total 94 -> 6, confirming the pattern
    reaches llvm-cov. Implements `coverage-metadata-ignore`
    (from kozmotic's template feedback).

- `/release` workflow + save-point `/commit` + deploy tag-guard

    Backfed from jutro's template feedback. Split the
    commit/release workflow: `/commit` becomes a save-point
    (review + diary + CHANGELOG `[Unreleased]`, no version
    bump, no `Cargo.lock` sync, no auto-validate), and a new
    `/release` command is the sole version-bumper -- it
    infers the bump from the accumulated `[Unreleased]`
    entries, promotes them to a dated section, runs `validate`
    as the release gate, commits the bookkeeping, and creates
    an annotated tag. Added `xtask/src/deploy_guard.rs`: a
    release-tag guard that refuses `cargo xtask deploy` unless
    `HEAD` is on a `vX.Y.Z` annotated tag matching
    `Cargo.toml` with a clean tree, with a `[section]`-aware
    TOML version parser so a dependency's `version` cannot
    shadow `[package]`. The pure parse + decision are
    unit-tested; the git queries are thin. Generalized
    jutro's `cargo generate-lockfile` to `cargo update -p
    rustbase` (the former trips `dep-age-check`).

- `/html-report` command for self-contained local HTML pages

    Backfed from jutro. A `/html-report` command plus its
    single-source-of-truth design asset
    `docs/ai-agents/html-report-template.html` produce a
    polished, self-contained local HTML page (all CSS/JS
    inlined, images as `data:` URIs, CSP-`<meta>`-enforced,
    dual light/dark theming, one `--fs-*` type scale) opened
    in the browser -- never a cloud Artifact. Fetched content
    is treated as data and HTML-escaped; the output slug is
    sanitized to `[a-z0-9-]+`.

### 2026-07-15

- `cargo xtask dep-preflight` -- pre-compile cooldown
  remediation (v0.15.0)

    Surfaced from clockdump's template feedback: the existing
    `dep-age-check` gate is *post-resolution* -- by the time it
    fails, `cargo` has already downloaded and compiled the
    fresh crates, running their build scripts on the host. The
    gate protects the committed lockfile, not the build
    machine. The insight that made a stable-toolchain fix
    possible: cargo resolves in three phases (resolve -> fetch
    -> compile), and only the last runs third-party code, so
    the full transitive version set is knowable from the index
    alone -- and an *old* parent still pulls *fresh* children,
    because dependency edges are semver ranges resolved to the
    newest match at resolve time. `dep-preflight` reads the
    changed crates (same `HEAD` diff as the gate) and pins each
    one still within the cooldown down to its newest aged
    version via `cargo update --precise`, looping until the set
    is aged or no aged version fits the resolved requirements.
    Every step is index-only, so no build script runs until the
    tree is clean. It is front-door only (opt-in; cannot
    intercept a bare `cargo build`) -- the only automatic
    protection is cargo's in-resolver `-Zmin-publish-age`
    (nightly), noted in CLAUDE.md as the migration target once
    it stabilizes on stable. The pin-and-re-resolve loop is
    fully unit-tested via an injected `Io` of fake closures;
    the git/curl/`cargo update` shell-outs are thin. Rust /
    crates.io only; the npm tree stays with `/update-deps`.

- `/update-deps` command for cooldown-aware upgrades (v0.14.0)

    Codified the third-party upgrade workflow (used earlier
    the same day to refresh the deps) into a repeatable
    `/update-deps` command spanning both ecosystems. Its
    tricky step -- "pin to the newest version outside the
    cooldown" -- is backed by a new
    `cargo xtask dep-age ... --latest-aged` mode rather than
    ad-hoc shell date-parsing, keeping the non-trivial logic
    in `xtask` (unit-tested) per the repo convention. The
    command assesses both ecosystems, asks scope (safe vs
    majors), re-pins any `cargo update` picks that landed
    within the window, runs the frontend major-bump
    procedure (targeted install; wipe only on `ERESOLVE`;
    restore cwd before `cargo xtask`), verifies with
    `validate`, reports held-back-as-fresh items with their
    age-out dates, and hands off to `/commit`.

- Dependency-cooldown enforced as a `validate` gate (v0.13.0)

    Promoted the 14-day dependency cooldown from an on-demand
    check to an automatic gate. New `cargo xtask dep-age-check`
    (validate step 11) diffs the working-tree `Cargo.lock` and
    `frontend/package-lock.json` against `HEAD` and cooldown-
    checks **only** the registry dependencies newly added or
    version-bumped there -- so it costs nothing (no network) on
    a commit that leaves the lockfiles untouched and fires
    exactly when a dependency is adopted. A *whole-tree* gate
    was deliberately avoided (it would flag every already-locked
    version on every routine update); that scoping is what made
    the reversal of yesterday's "never a validate gate" note
    (commit `4602285`) worthwhile, and the module doc now
    explains it. Local workspace crates (no `source`) and git
    deps are excluded -- only `source = "registry+..."` crates
    and npm entries with an `http` `resolved` URL are checked --
    so a version bump like this one does not spuriously warn.
    Like the audit gate, an unreachable registry / missing
    baseline degrades to a warning, not a failure. The
    `RUSTBASE_DEP_AGE_ALLOW=name@version` env var is the
    auditable escape hatch for a justified fresh adoption or a
    security fix.

### 2026-07-14

- Backfeed jutro Stage 6: skill/workflow changes

    Reworked the review workflow (items 36-40, no version
    bump -- tooling). Extracted the Red Team + Artisan
    prompts into a shared `.claude/commands/code-reviewers.md`
    referenced by both `/commit` and `/implement` (so the
    pre-launch and commit-time reviews are identical), and
    gave the Red Team prompt a historical-context lens
    (`git log` per touched file -> flag un-acknowledged
    reversals / churny surfaces). `/commit` now auto-applies
    mechanical findings (announced) and escalates only on six
    thresholds instead of prompting on every nit. Retired the
    counter-based four-log review system: deleted
    `redteam-resolved.md` / `artisan-resolved.md`, and the
    open logs became deferred-only backlogs with
    self-describing date-slug IDs (`rt-YYYY-MM-DD-slug`, no
    `Next ID`) -- a fixed finding's record now lives in its
    commit message. `/retrospect` gained a 4th "Cleanup"
    bucket (stale/duplicate canon + memory) with a periodic
    broader-canon-scan trigger. All opted into explicitly.

- Backfeed jutro Stage 4 items 18 + 19: supply-chain tooling (v0.12.0)

    `cargo xtask audit` -- RUSTSEC (`cargo audit`) + `npm
    audit` gate, fails on vulnerabilities only (advisory
    warnings informational), wired into `validate` as step 10
    (validate now needs cargo-audit + network). Landed on the
    clean tree after the Vite 8 bump cleared the npm highs;
    passes at `cargo: 0 vuln, 1 warn (unsound anyhow), npm: 0
    vuln`. `cargo xtask dep-age <npm|cargo> <pkg> [ver]` --
    on-demand cooldown helper; queries the registries via
    `curl` (no HTTP dep added to xtask) and ages the publish
    date with a dependency-free Hinnant civil-day count,
    failing under 14 days. Pure JSON/date helpers unit-tested
    with fixtures. Cooldown convention documented in a new
    CLAUDE.md "Supply-chain hygiene" section. Deviated from
    jutro's spec: no wait-timeout subprocess bound (would add
    a dep; noted as future hardening).

- Backfeed jutro Stage 4 item 23: Vite 8 (Rolldown) upgrade

    No version bump (toolchain). `vite ^7->^8`,
    `@sveltejs/vite-plugin-svelte ^6->^7` (svelte floor
    raised to `^5.46.4`), `vitest ^4.1.7`, dropped the
    `hot:false` vitest option. Clean install (per the
    clean-before-major-bump practice; reparse-scan-guarded
    `rm -rf node_modules` first). Sequenced deliberately
    *before* the audit gate (item 18) because the old
    `vite 7.x` was the source of the 3 high `npm audit`
    advisories -- the upgrade brings the tree to
    `found 0 vulnerabilities`, so the hard audit gate can
    land clean next. Verified: build (~130ms via Rolldown),
    svelte-check (0 errors), vitest (1 passed), and the e2e
    smoke suite on Vite 8.1.4 (5/5).

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
