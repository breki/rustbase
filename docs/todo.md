# TODO

Project work queue.

- `/todo <text>` captures a new item with a generated slug.
- `/todo` (no arguments) lists pending slugs.
- `/implement <slug>` plans and implements a pending item.
- `/implement` (no arguments) lists pending items and asks
  which to act on.

While an item is in flight, `/implement` keeps a working
planning doc at `docs/issues/<slug>.md` (problem, plan,
decisions). When the item is done, anything durable is
promoted to a reference doc or a code comment and the
planning doc is removed.

## Pending

<!-- Items captured by /todo land here. -->

- **xtask-strip-web** -- Ship `cargo xtask strip-web`
  as a one-shot in-place mutation that converts the
  template into a CLI-only project. Touches:
  `crates/rustbase-web/` (delete), `frontend/` (delete),
  `Cargo.toml` workspace members (remove web crate),
  `.github/workflows/` (remove frontend job),
  `build.ps1` (remove Invoke-Dev/Invoke-Frontend/
  Invoke-E2E functions and dispatch branches),
  `README.md` + `llms.txt` (drop web-app sections),
  `CLAUDE.md` (drop Frontend Development + E2E Testing
  sections), `scripts/e2e.sh` + `scripts/kill-servers.sh`
  (delete, orphaned without frontend), `.ports.sample`,
  and `playwright.config.ts` + root `tsconfig.json`.
  Requires a git-clean precondition check (irreversible
  in-place edit). Should land with its own xtask tests
  exercising the file-mutation logic against a fixture
  workspace. Significant scope -- worth a focused
  session and a `docs/issues/xtask-strip-web.md`
  planning doc up front. Sourced from kozmotic's
  template feedback (2026-05-04).

- **coverage-excluded-lines** -- report lines excluded by the coverage regex
  From bombyx template feedback (2026-08-18,
  tf-2026-08-18-coverage-gate-cannot-see-src-bin). cargo xtask coverage prints
  e.g. "98.0%" but never says the IGNORE_REGEX hid all of src/bin/ from the
  denominator, so the number reads as a statement about the whole project. Have
  coverage report what was excluded -- a file/line count beside the percentage,
  e.g. "98.0% of 2,102 lines; 640 lines in 1 file excluded". Design decision:
  computing the excluded count needs either a second llvm-cov pass without the
  ignore filter (doubles the slowest gate) or counting the excluded files
  line-by-line; MODULE_THRESHOLD has the same blind spot. Not a mechanical port
  -- bombyx only suggested it, never implemented it.

- **todo-edit-subcommand** -- Add `cargo xtask todo edit` to revise a captured
  entry
  The `/todo` rules forbid hand-editing `docs/todo.md`, but `todo` offers only
  list/add/done, so a correction to a captured item forces a hand edit or a
  duplicate slug describing the same work. Add `todo edit --slug <slug>` that
  rewrites the summary, the body, or both through the existing wrap-and-render
  path, so a revised entry is indistinguishable from a fresh one; the reader
  already tolerates a multi-paragraph body, and the writer must preserve that.
  Failing that, soften the rule in `.claude/commands/todo.md` to name the edit
  the tooling cannot make. Sourced from bombyx template feedback (2026-08-30).

- **drop-npm-cooldown-pins** -- Remove the npm cooldown pins after 2026-11-01
  frontend/package.json pins vitest exactly at 4.1.11 and overrides postcss
  (8.5.29) and undici (7.30.0), because the newer releases were inside the
  14-day cooldown on 2026-10-19 (vitest 4.1.12 and undici 7.30.1 clear on
  2026-10-29, postcss 8.5.31 on 2026-11-01). Left in place they freeze those
  packages. After 2026-11-01: restore the vitest caret range, delete the
  overrides block, run /update-deps for the frontend, and confirm npm audit and
  dep-age-check stay clean.

## Done

- [**template-tooling-cli-redesign**](issues/template-tooling-cli-redesign.md)
  -- Moved template-maintenance delta determination and log
  bookkeeping into unit-tested `cargo xtask` commands
  (`backfeed-diff`, `backfeed-record`, `feedback-add`,
  `sync-candidates`) plus a machine-owned `backfeed-ledger.toml`
  watermark; the LLM keeps only judgment. Rewrote
  `/template-backfeed`, `/template-improve`, `/template-sync`.
  (2026-07-16)

- [**coverage-metadata-ignore**](issues/coverage-metadata-ignore.md)
  -- Coverage `--ignore-filename-regex` now merges
  `[workspace.metadata.coverage] ignore` from the root
  `Cargo.toml`; derived projects exclude hardware-bound modules
  via manifest config instead of forking `coverage.rs`.
  (2026-07-16)

- [**changelog-version-drift**](issues/changelog-version-drift.md)
  -- Backfilled dated CHANGELOG sections 0.10.1-0.15.0 from git
  history; `[Unreleased]` now holds only genuinely-unreleased
  work. (2026-07-16)

<!-- Completed items are moved here by /implement during
     finalisation, linked to their issue doc. -->
