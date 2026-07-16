# TODO

Project work queue.

- `/todo <text>` captures a new item with a generated slug.
- `/todo` (no arguments) lists pending slugs.
- `/implement <slug>` plans and implements a pending item.
- `/implement` (no arguments) lists pending items and asks
  which to act on.

Each implemented item gets a planning doc at
`docs/issues/<slug>.md` that captures the problem statement,
plan, decisions, and outcome.

## Pending

<!-- Items captured by /todo land here. -->

- **template-tooling-cli-redesign** -- Make `/template-sync`
  and `/template-backfeed` scalable by moving delta
  determination and log bookkeeping out of the LLM and into
  deterministic, unit-tested `cargo xtask` commands. The LLM
  keeps only the judgment (categorize, decide apply/skip, merge
  code); it no longer linearly reads/parses growing markdown
  logs.

  **Problem.** `/template-backfeed` has no watermark: every run
  re-reads the downstream's entire `template-feedback.md`
  (jutro is already 2174 lines) and cross-references the
  template's entire Resolved section. Cost grows without bound.
  `/template-sync` is already SHA-delta-based
  (`last-synced..template/main`) so its determination is fine,
  but template-internal bookkeeping files (CHANGELOG.md, the
  feedback file) show up as sync candidates, so their growth
  becomes review noise on every downstream sync.

  **Principle to record in CLAUDE.md.** Template-bookkeeping
  deltas and log mutations come from `cargo xtask`, not from an
  LLM scan of the file. Reserve the LLM for judgment.

  **Phase 1 -- backfeed watermark (biggest win).**
  - New template file `docs/developer/backfeed-ledger.toml`,
    one table per downstream: `watermark` (newest entry date
    already evaluated), `downstream-head` (SHA, provenance,
    read from `<ds>/.git`), `last-run`. Seed it now with
    `jutro = 2026-07-14` and `clockdump = 2026-07-15` so their
    history is never re-scanned.
  - `cargo xtask backfeed-diff <downstream-path>` -- read the
    downstream feedback file + ledger watermark, print only the
    entries newer than the watermark. Parse on date-bearing
    headers (`##`/`###` containing an ISO date) so it tolerates
    today's heterogeneous downstream formats; cut by watermark
    date. This replaces the LLM's full-file read.
  - `cargo xtask backfeed-record <downstream> --watermark
    <YYYY-MM-DD> [--head <sha>]` -- update the ledger.
  - Rewrite `.claude/commands/template-backfeed.md` to call
    these: first run offers a bootstrap choice ("start from
    now" vs "full history once", mirroring sync's bootstrap);
    normal run acts only on `backfeed-diff` output, then calls
    `backfeed-record`. Keep the Resolved cross-ref only over
    the (now small) candidate set as a same-day-boundary safety
    net.

  **Phase 2 -- deterministic feedback append + stable IDs.**
  - Define a strict entry grammar: `### tf-<yyyy-mm-dd>-<slug>
    -- <title>` under `## <Section>` (mirrors the RT/AQ-log
    convention already in `docs/developer/`).
  - `cargo xtask feedback-add --section
    <open|resolved|suggestion> --title "..."` (body on stdin or
    `--body-file`) -- generate the `tf-<date>-<slug>` ID, insert
    at the top of the section with consistent structure. LLM
    supplies pre-wrapped prose; the command owns ID + placement
    + dedup.
  - Rewrite `.claude/commands/template-improve.md` to call it.
    Old freeform entries still parse under Phase 1's
    date-header cut; IDs add per-entry precision going forward
    (no migration needed).

  **Phase 3 -- sync exclude set.**
  - One tested `const` in xtask for the template-internal,
    never-sync paths: `CHANGELOG.md`,
    `docs/developer/{template-feedback,backfeed-ledger,DIARY,
    redteam-log,artisan-log}.md`, `docs/issues/`.
  - `cargo xtask sync-candidates` -- run `git diff --name-status
    <last-synced>..template/main`, filter the exclude set,
    categorize by path -> clean candidate table.
  - `.claude/commands/template-sync.md` step 5 consumes it, so
    growing bookkeeping files never appear as sync candidates.

  **Notes.** All xtask commands ship in the template and are
  available in derivatives (backfeed runs from the template
  repo against a downstream path; sync runs from a downstream).
  Each phase is independently shippable (own commit). Every
  command gets unit tests for its parse/dedup/format logic
  (xtask is excluded from the coverage gate, but test the pure
  parsing regardless). Design decision still open: whether the
  watermark ledger should be a dedicated file (proposed) or
  derived from the template's own Resolved "Surfaced from
  <ds> (<date>)" lines -- dedicated file preferred because it
  can also record skip/defer decisions, but confirm at
  implement time. Archival/rollup of old Resolved entries is
  explicitly out of scope: the watermark decouples
  determination cost from file length, so the logs may keep
  growing. Captured 2026-07-16 from a design discussion.

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

## Done

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
