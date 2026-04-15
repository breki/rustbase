# Development Diary

This diary tracks functional changes to the codebase in
reverse chronological order.

---

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
