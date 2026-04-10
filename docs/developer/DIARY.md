# Development Diary

This diary tracks functional changes to the codebase in
reverse chronological order.

---

### 2026-04-10

- Address template feedback from hoard project (v0.1.1)

    Coverage now excludes `xtask` crate and binary
    `main.rs` entry points via `--exclude` and
    `--ignore-filename-regex`. Added `cargo xtask dupes`
    for code duplication detection (6% threshold,
    `code-dupes`). Added `/template-improve` command,
    TDD guidance, and enforced `/commit` for all commits.
