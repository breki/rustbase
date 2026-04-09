# rustbase

Opinionated Rust project template with Claude Code
integration, quality gates, and CI/CD.

## What's included

- **Cargo workspace** with `crates/<name>` + `xtask`
- **Optional web app**: Axum backend + Svelte 5/Vite
  frontend (delete if not needed)
- **Claude Code** configuration:
  - `CLAUDE.md` project guidance
  - Stop hook running validation on modified Rust files
  - `/commit` skill with Red Team + Artisan code review
  - `/todo` skill for TODO.md processing
- **xtask** build automation:
  - `cargo xtask validate` (fmt + clippy + tests +
    coverage)
  - `cargo xtask test [filter]`
  - `cargo xtask clippy`
  - `cargo xtask fmt`
  - `cargo xtask coverage` (90% threshold)
- **GitHub Actions**:
  - CI: fmt, clippy, tests on Linux/Windows/macOS
  - Release: 5-target builds with checksums
- **Code quality**:
  - `#[deny(warnings)]`, `#[forbid(unsafe_code)]`
  - Clippy pedantic + perf
  - 90% line coverage minimum
  - Per-module 85% coverage floor
- **Conventions**:
  - Conventional Commits with AI-Generated footer
  - Semantic Versioning
  - Keep a Changelog format
  - Development diary for tracking changes
  - Code review finding logs (Red Team + Artisan)
  - LF line endings enforced
  - 80-char line width (code and markdown)

## Using the template

1. Click **Use this template** on GitHub (or clone)
2. Search-and-replace `rustbase` with your project name
   in:
   - `Cargo.toml` (workspace)
   - `crates/rustbase/Cargo.toml` (package name,
     repository URL)
   - `crates/rustbase-web/Cargo.toml` (if keeping web)
   - `crates/rustbase/src/bin/rustbase/main.rs`
   - `.github/workflows/release.yml` (binary name,
     archive name)
   - `CLAUDE.md` (crate path references)
   - `.claude/commands/commit.md` (crate path)
   - `frontend/package.json` (if keeping web)
3. Rename `crates/rustbase/` to `crates/<your-name>/`
4. Update `CLAUDE.md` project overview
5. Update `README.md`

### Don't need the web app?

Delete these and you're left with a pure CLI template:

1. `crates/rustbase-web/`
2. `frontend/`
3. Remove `"crates/rustbase-web"` from workspace
   `members` in `Cargo.toml`

## Development

### CLI only

```bash
cargo xtask validate          # full quality check
cargo run -p rustbase         # run CLI
```

### Web app

```bash
cd frontend && npm install    # first time
cargo run -p rustbase-web &   # backend on :3000
cd frontend && npm run dev    # frontend on :5173
```

Open http://localhost:5173. Vite proxies `/api` requests
to the Axum backend.

For production: `cd frontend && npm run build`, then
`cargo run -p rustbase-web -- --frontend frontend/dist`.

## Prerequisites

- Rust (stable, via `rust-toolchain.toml`)
- `cargo-llvm-cov` for coverage:
  `cargo install cargo-llvm-cov`
- Node.js (for frontend, if using web app)

## License

MIT
