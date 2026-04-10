# Development Diary

This diary tracks functional changes to the codebase in
reverse chronological order.

---

### 2026-04-10

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
