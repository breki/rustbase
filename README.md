# rustbase

Opinionated Rust project template with Claude Code
integration, quality gates, and CI/CD.

## What's included

- **Cargo workspace** with `crates/<name>` + `xtask`
- **Optional web app**: Axum backend + Svelte 5/Vite
  frontend (delete if not needed)
- **Claude Code** configuration:
  - `CLAUDE.md` project guidance, including a
    behaviour-change-vs-structural-addition refinement
    of the TDD rule, an edition-2024 migration
    appendix, the workspace-vs-xtask `unsafe_code`
    override recipe, and a version-source-of-truth
    convention
  - Stop hook running validation on modified Rust files
  - `/commit` -- versioning, diary, CHANGELOG, parallel
    Red Team + Artisan code reviews with **cross-
    confirmed-finding** detection, and a post-commit
    workflow retrospective
  - `/retrospect` -- standalone workflow retrospective
    (Efficiency / Quality / Speed buckets,
    `[trivial]`/`[propose]` tags, auto-apply offer for
    trivial fixes). Invoked automatically by `/commit`;
    also callable manually mid-session
  - `/todo` -- capture work items with generated slugs
    into `docs/todo.md` (capture only, never
    implements)
  - `/implement` -- plan + execute a captured item.
    Phase 1 writes a planning doc at
    `docs/issues/<slug>.md`; Phase 2 codes with TDD;
    Phase 3 validates and hands off to `/commit`
  - `/template-sync` -- pull improvements from the
    upstream rustbase template into a derived
    project; cross-references the project's open
    divergences in `template-feedback.md` so
    documented differences aren't re-proposed every
    sync
  - `/template-improve` -- log feedback about the
    upstream template (open / resolved / suggestions
    to flow back -- three-section lifecycle)
  - `/architect` and `/web-dev` domain skills
- **xtask** build automation:
  - `cargo xtask validate` (fmt + clippy + tests +
    coverage + duplication + frontend type-check;
    coverage failures include uncovered-line ranges
    per failing module)
  - `cargo xtask test [filter]`, `clippy`, `fmt`,
    `check` (fast compile-only)
  - `cargo xtask coverage` (90% workspace floor, 85%
    per module; structured failure reporting via
    `CoverageFailure` enum)
  - `cargo xtask dupes` (6% threshold)
  - `cargo xtask deploy` / `deploy-setup` -- one-shot
    deployment to a remote Linux + systemd host with
    sandboxed service unit, frontend bundling, and
    input-validated config
  - `cargo xtask clean-cache` -- drop stale
    `target/{debug,release}/incremental/` content
    (symlink-safe; continues past locked files)
- **GitHub Actions**:
  - CI: fmt, clippy, tests on Linux/Windows/macOS
  - Release: 5-target builds (both CLI + web binaries)
    with frontend dist and checksums
- **Code quality**:
  - `#[deny(warnings)]`, `#[forbid(unsafe_code)]`
  - Clippy pedantic + perf
  - 90% line coverage minimum
  - Per-module 85% coverage floor
  - Code duplication <= 6% (production code only)
- **Build profile defaults**:
  - `[profile.release]` tuned for iteration speed
    (`incremental = true`, `codegen-units = 256`).
    Documented override guidance for performance-
    critical targets
- **Conventions**:
  - Conventional Commits with AI-Generated footer
  - Semantic Versioning
  - Keep a Changelog format
  - Development diary for tracking changes
  - Code review finding logs (Red Team + Artisan,
    open + resolved pair)
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
3. `e2e/`
4. `playwright.config.ts`
5. Root `package.json`
6. Remove `"crates/rustbase-web"` from workspace
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

### E2E tests

```bash
npx playwright test           # auto-starts servers
npx playwright test --ui      # interactive mode
```

### PowerShell

```powershell
.\build.ps1 validate          # full quality check
.\build.ps1 e2e               # E2E tests
.\build.ps1 build             # everything
```

## Prerequisites

- Rust (stable, via `rust-toolchain.toml`)
- `cargo-llvm-cov` for coverage:
  `cargo install cargo-llvm-cov`
- `code-dupes` for duplication checks:
  `cargo install code-dupes`
- Node.js 22+ (for frontend, if using web app)

## License

MIT
