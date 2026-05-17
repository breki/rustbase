# Ledgerstone template improvements -- plan & tracker

Source: `D:\src\ledgerstone\main\docs\developer\template-feedback.md`,
section **"Suggestions to flow back to the template"**.

Ledgerstone was last synced from rustbase at v0.4.0 (commit
`076cf44`). Everything in the source section is a candidate to
flow back upstream into this template.

Each item below has:
- **Scope** -- rough size estimate
- **Status** -- `todo`, `done`, `skip`, or `defer`
- **Target** -- files most likely to change
- **Notes** -- decisions, blockers, or implementation hints

Update the status column as items land. Reference this file from
the eventual commit message(s) so the trail is preserved.

---

## Code changes -- small (quick wins)

### 1. `[profile.release]` defaults for fast iteration

- **Scope:** 5-line block in workspace `Cargo.toml` + CLAUDE.md
  note on the trade-off.
- **Status:** **done (Cargo.toml)**, CLAUDE.md note deferred to
  Batch C docs pass.
- **Target:** `Cargo.toml`, `CLAUDE.md`
- **Notes:** Ship `incremental = true, codegen-units = 256`.
  Document that perf-critical targets should override. Ledgerstone
  measured 8min -> 1m06s on `restart-prod`.

### 2. `scripts/deploy-prod.ps1` swallows npm output via `Out-Null`

- **Scope:** Two-line edit (replace `2>&1 | Out-Null` with
  `2>&1 | Tee-Object -FilePath deploy.log`).
- **Status:** **skip -- not applicable**. rustbase v0.5.0 moved
  deploy into xtask. `xtask/src/deploy_remote.rs` uses
  `Command::status()` for ssh/scp which streams stdout/stderr
  through naturally -- there is no `Out-Null` suppression to
  fix. The deploy-log persistence problem (a tail-able artefact
  surviving the run) is a different need; if it ever matters,
  revisit as a new item.
- **Target:** n/a
- **Notes:** Resolved by deploy-as-xtask migration.

### 3. `cargo xtask check-duplication` chokes on stale `jscpd-report/`

- **Scope:** One-line `fs::remove_dir_all(report_dir).ok()`
  pre-clean in `xtask/src/duplication.rs::run_check()`, plus
  always surfacing captured stderr in the error message.
- **Status:** **skip -- not applicable**. rustbase uses
  `code-dupes` (a Rust tool invoked with `--exclude-tests check
  --max-exact-percent`), not `jscpd`. There is no
  `jscpd-report/` directory to stale-clean. `xtask/src/dupes.rs`
  uses `.status()` (streaming) and surfaces the install hint on
  spawn failure.
- **Target:** n/a
- **Notes:** Different tool, different failure mode -- the
  Ledgerstone fix has no analogue here.

### 4. Aggregated `npm run check:all` and `npm run fix`

- **Scope:** Two entries in `frontend/package.json` scripts block.
- **Status:** **done**
- **Target:** `frontend/package.json`
- **Notes:**
  ```json
  "fix": "prettier --write . && eslint . --fix",
  "check:all": "npm run check && npm run lint && npm run format:check && npm run test && npm run build"
  ```
  Discoverable via `npm run` listing -- helps agents avoid
  partial command chains.

### 5. `validate` runs the test suite twice

- **Scope:** ~5-line orchestration change in `xtask/src/validate.rs`.
- **Status:** **done** (slight variant: rustbase's coverage runs
  `--workspace --exclude xtask`, so Test now runs `-p xtask` only
  instead of being conditionally skipped; same end result of no
  duplication, with xtask still covered).
- **Target:** `xtask/src/validate.rs`, `xtask/src/test_cmd.rs`
  (new `test_check_xtask` helper).
- **Notes:** Conditionally skip the standalone `Test` step when
  `Coverage` will run (since `cargo llvm-cov` already runs the
  full suite). Keep `Test` standalone for the `--skip-coverage`
  path. Step-count must be computed dynamically. Ledgerstone
  measured ~95-155s -> ~58s. **Highest-yield/lowest-effort
  optimisation in the list.**

---

## Code changes -- medium

### 6. New `cargo xtask clean-cache` command

- **Scope:** ~75 LOC implementation + ~90 LOC tests in a new
  `xtask/src/clean_cache.rs` module, plus wiring in
  `xtask/src/main.rs` / command enum.
- **Status:** **done** -- v0.7.0. Two hardenings beyond
  the Ledgerstone original: symlink/junction safety
  (RT-038, regression test included) and continue-on-
  error per-entry deletion (RT-039). Shared utilities
  (`dir_size`, `fmt_bytes`, `temp_scratch`) live in
  `helpers.rs`.
- **Target:** `xtask/src/clean_cache.rs` (new),
  `xtask/src/helpers.rs`, `xtask/src/main.rs`.
- **Notes:** Walks `target/debug/incremental/` and
  `target/release/incremental/`, deletes contents (preserves
  the dirs themselves), reports bytes freed. Manual invocation
  only -- never auto-wired. Ledgerstone freed 32.8 GB on first
  run.

### 7. Auto-print coverage detail on validate failure

- **Scope:** Medium change in `xtask/src/coverage.rs`:
  parse llvm-cov JSON segments (rustbase uses JSON, not
  cobertura XML like Ledgerstone) for failing modules'
  uncovered-line ranges and append to the error.
- **Status:** **done** -- v0.8.0. Implemented as a
  typed Segment struct + structured CoverageFailure
  enum; presentation lives in `format_failure`. Four
  review findings (RT-040/041 cross-confirmed, AQ-035/036/037/038)
  fixed pre-commit.
- **Target:** `xtask/src/coverage.rs`,
  `xtask/src/validate.rs`, `xtask/Cargo.toml` (serde
  derive).
- **Notes:** Output shape matches the Ledgerstone
  suggestion: `module: pct%` then indented
  `uncovered: <ranges>`. Range computation handles
  trailing segment explicitly to avoid the
  `windows(2)` drop.

### 8. `cargo xtask validate` unified-with-flags pattern

- **Scope:** Larger refactor: collapse `validate` + `quick-validate`
  duality into one command with `--skip-coverage`,
  `--skip-duplication`, `--skip-frontend`, `--e2e`, `--all` flags.
- **Status:** todo (possibly defer)
- **Target:** `xtask/src/validate.rs`, `xtask/src/main.rs`.
- **Notes:** Step count + `[N/T]` prefixes scale dynamically.
  Ledgerstone already has this shape -- can mostly port. Verify
  rustbase v0.5.0 hasn't already moved in this direction.

---

## Documentation-only changes

### 9. `.template-sync.toml` header comment linking to skill docs

- **Scope:** ~3-line comment block at top of the file.
- **Status:** **done** -- v0.8.x docs batch.
- **Target:** `.template-sync.toml`.
- **Notes:** Expanded the header into a ~15-line
  comment block describing the file's role, the skill
  that manages it, the cross-reference to
  `template-feedback.md`, and the sibling
  `/template-improve` skill.

### 10. `unsafe_code = forbid` workspace vs xtask override recipe

- **Scope:** CLAUDE.md appendix or workspace lints doc.
- **Status:** **done** -- v0.8.x docs batch.
- **Target:** `CLAUDE.md` ("Workspace lints and xtask
  overrides" section).
- **Notes:** Documents the local-override recipe with
  a concrete `xtask/Cargo.toml [lints.rust]` block
  showing `unsafe_code = "allow"` scoped to the xtask
  crate only.

### 11. Edition-2024 migration appendix

- **Scope:** CLAUDE.md appendix.
- **Status:** **done** -- v0.8.x docs batch.
- **Target:** `CLAUDE.md` ("Edition-2024 migration
  notes" section).
- **Notes:** Lists the four mechanical fixes with
  before/after examples: `unsafe extern` blocks,
  dropped `ref` patterns, `r#gen` raw identifier, and
  the let-chain collapse follow-up.

### 12. Version-source-of-truth convention for README

- **Scope:** Convention note in CLAUDE.md.
- **Status:** **done** -- v0.8.x docs batch.
- **Target:** `CLAUDE.md` ("Version source of truth"
  section).
- **Notes:** Documents the `<!-- version: X.Y.Z -->`
  sentinel pattern and the `env!("CARGO_PKG_VERSION")`
  / Vite alternatives. rustbase's README currently has
  no version mentions, so no in-repo drift to fix --
  this is a forward-looking convention.

### 13. Restructure `template-feedback.md` into three sections

- **Scope:** Edit `template-feedback.md` scaffolding and update
  `.claude/commands/template-improve.md`.
- **Status:** **done** -- v0.8.x docs batch.
- **Target:** `docs/developer/template-feedback.md`,
  `.claude/commands/template-improve.md`.
- **Notes:** New file shape uses three sections (Open
  divergences / Resolved / Suggestions to flow back).
  Existing entries reclassified: build.ps1
  double-validate and Playwright fixture-isolation
  remain in Open; everything else (most entries) moved
  to Resolved with one-line resolution summaries.
  `/template-improve` skill instructions updated to
  route new entries by section.

---

## Skill / workflow changes

### 14. Template-sync should cross-reference template-feedback

- **Scope:** Step 5 of `/template-sync.md` -- read
  `template-feedback.md`, parse **Open divergences**, flag
  conflicting incoming changes as `skip` with the divergence
  reason inline.
- **Status:** todo
- **Target:** `.claude/commands/template-sync.md`
- **Notes:** Reduces churn at review time. Ledgerstone's local
  copy already has this enhancement -- port the diff.

### 15. Workflow retrospective as final `/commit` step

- **Scope:** Add step 12 to `.claude/commands/commit.md`: post-
  commit retrospective with Efficiency / Quality / Speed
  buckets, findings tagged `[trivial]` / `[propose]`, recursive-
  skip carve-out for workflow-only commits.
- **Status:** todo
- **Target:** `.claude/commands/commit.md`
- **Notes:** Runs *after* commit lands so it can't block
  shipping. Findings ephemeral by default. Ledgerstone applied
  this and surfaced 5 process improvements in 2 sessions.

### 16. Cross-reviewer agreement signal in `/commit`

- **Scope:** In step 5 of `/commit`, scan Red Team and Artisan
  reports for findings on same file:line or root cause; surface
  under a `Cross-confirmed` heading.
- **Status:** todo
- **Target:** `.claude/commands/commit.md`
- **Notes:** Empirical signal: cross-confirmed findings had a
  higher fix-rate than unique findings in Ledgerstone sessions.

### 17. `/implement` Phase 2 TDD strictness refinement

- **Scope:** Refine Phase 2 rule in `.claude/commands/
  implement.md` (if such a command exists in rustbase) to
  distinguish **behaviour change** (strict TDD applies) vs
  **structural addition** (test + impl together).
- **Status:** todo (verify command exists)
- **Target:** `.claude/commands/implement.md` -- confirm
  presence first. rustbase may not ship this command.
- **Notes:** Strict TDD on `xtask/src/clean_cache.rs`-shaped
  additions is theatre. Self-contained module + unit tests
  written together is the correct pattern.

---

## Out of scope / explicit non-imports

These items in the source feedback describe Ledgerstone-specific
divergences that should **not** flow back:

- Stop hook running clippy-only (project-specific perf trade-off).
- Extra `PreToolUse` / `PostToolUse` hooks (worth mentioning as
  optional add-ons in template docs, but not shipping by default).
- Orphan `check-coverage-hook.sh` (Ledgerstone's own cleanup item).
- Workspace-vs-xtask `unsafe_code` divergence (already covered as
  item #10 doc note above).
- Prettier config divergence (project-specific style preservation).
- `currency` state placeholder in TransactionsList (Ledgerstone-
  specific code).
- Crate split divergence (intentional permanent divergence).
- No-CI policy (project-specific).

---

## Suggested batching for commits

To keep diffs reviewable, split landings roughly:

1. **Batch A -- quick code wins** (items 1, 2, 3, 4, 5) ->
   one commit, "feat: apply ledgerstone quick-win improvements".
2. **Batch B -- new xtask features** (items 6, 7, optionally 8)
   -> one or two commits.
3. **Batch C -- documentation** (items 9, 10, 11, 12, 13) ->
   one commit.
4. **Batch D -- skill / workflow** (items 14, 15, 16, 17) ->
   one commit per skill or one bundled commit.

Each commit should bump the version per SemVer (mostly MINOR for
features, PATCH for doc / xtask diagnostic fixes) and update
`CHANGELOG.md`.

After landing, update `template-feedback.md` in Ledgerstone to
move the corresponding entries from **Suggestions to flow back**
into a **Resolved** equivalent (or delete them), and bump
`.template-sync.toml` to the new rustbase commit SHA.
