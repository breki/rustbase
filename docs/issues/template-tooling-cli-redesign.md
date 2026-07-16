# template-tooling-cli-redesign

**Status:** Done
**Completed:** 2026-07-16
**Captured:** 2026-07-16
**Started:** 2026-07-16

## Problem

`/template-backfeed` and `/template-sync` currently push
growing markdown-parsing work onto the LLM. Backfeed has no
watermark: every run re-reads the downstream's entire
`template-feedback.md` (a real downstream is already 2174
lines) and cross-references the template's entire Resolved
section. Cost grows without bound. Sync is SHA-delta-based, so
its determination is fine, but template-internal bookkeeping
files (CHANGELOG.md, the feedback file, logs) surface as sync
candidates, adding review noise on every downstream sync.

Move the deterministic work -- delta determination, log
bookkeeping, exclude-set filtering -- out of the LLM and into
unit-tested `cargo xtask` commands. The LLM keeps only the
judgment: categorize, decide apply/skip, merge code.

## Context

- `xtask` crate: `main.rs` wires each subcommand via a clap
  `XCommand` enum; one module per command. Tests live in a
  `#[cfg(test)]` module at the bottom of each command file
  (see `dep_age.rs` for the parse-heavy pattern).
- `xtask/Cargo.toml` deps: `clap`, `serde`, `serde_json`, and
  `windows-sys` (windows-only). **No `toml` crate.**
  `coverage.rs` deliberately hand-parses the one TOML shape it
  needs (`[workspace.metadata.coverage] ignore = [...]`) via
  line-leading string scanning rather than pulling in `toml`.
- Shared helpers in `helpers.rs`: `workspace_root()`,
  `run_cargo_capture()`, step formatting, dir-size, reparse
  guards. No generic git-capture helper yet (each command runs
  git ad hoc).
- Log ID convention already in `docs/developer/`:
  `rt-<yyyy-mm-dd>-<slug>` (redteam-log),
  `aq-<yyyy-mm-dd>-<slug>` (artisan-log). Phase 2's
  `tf-<yyyy-mm-dd>-<slug>` mirrors this.
- `template-feedback.md` uses three `##` sections (Open
  divergences / Resolved / Suggestions to flow back). Entries
  are `### <date-or-topic> -- <title>` (heterogeneous: some
  date-led, some topic-led). Downstream files may be freeform.
- Commands live in `.claude/commands/*.md` with a frontmatter
  `allowed-tools` line; they will need `Bash(cargo xtask ...)`
  entries added for the new commands.
- This repo is the template (`origin` = `breki/rustbase`), so
  backfeed commands run here against a downstream path.

## Open questions

- Scope/sequencing: land all three phases this session (three
  commits) or start with Phase 1 only? -> see Decisions.
- Ledger storage: dedicated `backfeed-ledger.toml` vs derived
  from Resolved "Surfaced from <ds>" lines. -> see Decisions.
- TOML read/write for the ledger: hand-roll (matches
  `coverage.rs` precedent, ledger is machine-owned) vs add the
  `toml` crate. -> see Decisions.

## Plan

### Phase 1 -- backfeed watermark (biggest win)

1. New template file `docs/developer/backfeed-ledger.toml`,
   one `[<downstream>]` table each: `watermark`
   (newest entry date already evaluated, `YYYY-MM-DD`),
   `downstream-head` (SHA + provenance, read from
   `<ds>/.git`), `last-run`. Seed `jutro = 2026-07-14` and
   `clockdump = 2026-07-15`.
2. `xtask/src/backfeed.rs`:
   - Ledger read/write (parse + serialize, machine-owned
     format).
   - `backfeed_diff(downstream_path)` -- read the downstream
     feedback file + ledger watermark for that downstream;
     print only entries under date-bearing headers
     (`##`/`###` containing an ISO date) newer than the
     watermark. Tolerant of heterogeneous formats.
   - `backfeed_record(downstream, watermark, head?)` --
     upsert the ledger table and rewrite the file.
3. Wire `BackfeedDiff` / `BackfeedRecord` subcommands in
   `main.rs`.
4. Rewrite `.claude/commands/template-backfeed.md` to call
   these: first-run bootstrap choice ("start from now" vs
   "full history once"); normal run acts only on
   `backfeed-diff` output, then calls `backfeed-record`. Keep
   the Resolved cross-ref only over the (now small) candidate
   set as a same-day-boundary safety net.

### Phase 2 -- deterministic feedback append + stable IDs

5. Entry grammar: `### tf-<yyyy-mm-dd>-<slug> -- <title>`
   under `## <Section>`.
6. `xtask/src/feedback.rs`: `feedback_add(section, title,
   body)` (body via stdin or `--body-file`) -- generate the
   `tf-<date>-<slug>` ID, insert at the top of the section,
   dedup by ID. LLM supplies pre-wrapped prose; the command
   owns ID + placement + dedup.
7. Wire `FeedbackAdd` subcommand in `main.rs`.
8. Rewrite `.claude/commands/template-improve.md` to call it.
   Old freeform entries still parse under Phase 1's date cut.

### Phase 3 -- sync exclude set

9. One tested `const` in `xtask` of never-sync template
   -internal paths: `CHANGELOG.md`,
   `docs/developer/{template-feedback,backfeed-ledger,DIARY,
   redteam-log,artisan-log}.md`, `docs/issues/`.
10. `xtask/src/sync.rs`: `sync_candidates(last_synced)` --
    run `git diff --name-status <last-synced>..template/main`,
    filter the exclude set, categorize by path -> candidate
    table.
11. Wire `SyncCandidates` subcommand in `main.rs`.
12. `.claude/commands/template-sync.md` step 5 consumes it.

### Cross-cutting

- Record the principle in `CLAUDE.md`: template-bookkeeping
  deltas and log mutations come from `cargo xtask`, not from
  an LLM scan; reserve the LLM for judgment.
- Add `Bash(cargo xtask ...)` entries to the affected command
  frontmatter `allowed-tools`.

## Test strategy

All logic is pure string/parse work -- Rust unit tests in each
module's `#[cfg(test)]` block (the `dep_age.rs` pattern). No
frontend, no E2E. Key cases:

- **Ledger** round-trip: parse then serialize is stable;
  upsert updates an existing table and appends a new one;
  missing/unreadable ledger degrades gracefully.
- **backfeed-diff**: entries newer than watermark are kept,
  older/equal are dropped; date extracted from both `##` and
  `###` headers; heterogeneous/freeform headers tolerated;
  same-day boundary behaviour is explicit.
- **feedback-add**: `tf-<date>-<slug>` ID generation
  (slugification, collision suffix); inserts at section top;
  dedup by ID; section not found errors cleanly.
- **sync-candidates**: exclude set filters the named paths;
  categorization by path prefix; `--name-status` parsing
  (added/modified/deleted/renamed rows).

## Progress log

- **2026-07-16** -- Phase 1 landed: `today_iso` /
  `is_iso_date` / `extract_iso_date` date helpers in
  `helpers.rs`; `xtask/src/backfeed.rs` with hand-rolled
  ledger parse/serialize, `backfeed-diff`, `backfeed-record`;
  seeded `docs/developer/backfeed-ledger.toml`
  (jutro=2026-07-14, clockdump=2026-07-15); `main.rs` wiring;
  rewrote `template-backfeed.md`. Clippy + unit tests green;
  smoke-tested diff (37 entries, full history) and record
  (valid sorted TOML round-trip).
- **2026-07-16** -- Phase 2 landed: `xtask/src/feedback.rs`
  (`feedback-add`) with `slugify` / `make_id` / `format_entry`
  / `insert_entry`; `main.rs` wiring; rewrote
  `template-improve.md`. Smoke-tested insert at section top +
  idempotent re-add + `--body-file`; restored the file.
- **2026-07-16** -- Phase 3 landed: `xtask/src/sync.rs`
  (`sync-candidates`) with the `NEVER_SYNC` const,
  `is_excluded` / `categorize` / `parse_name_status` /
  `candidates` / `format_candidates`; `main.rs` wiring; rewrote
  `template-sync.md` step 5. Smoke-tested the happy path with a
  temporary `refs/remotes/template/main` (8 candidates, 4
  excluded), then removed the ref.
- **2026-07-16** -- Cross-cutting: recorded the determinism-vs
  -judgment principle + new commands in `CLAUDE.md`. Full
  `cargo xtask validate` green (11/11, coverage 100%, dupes 0%).

## Outcome

Shipped all three phases plus the cross-cutting principle. New
`cargo xtask` commands, each with pure-logic unit tests:

- **Phase 1** -- `xtask/src/backfeed.rs`: `backfeed-diff`
  (delta since the ledger watermark) and `backfeed-record`
  (advance it). Seeded `docs/developer/backfeed-ledger.toml`.
  Rewrote `.claude/commands/template-backfeed.md` (bootstrap
  choice + deterministic delta + watermark record).
- **Phase 2** -- `xtask/src/feedback.rs`: `feedback-add`
  mints `tf-<date>-<slug>` IDs, inserts at the section top,
  dedups. Rewrote `.claude/commands/template-improve.md`.
- **Phase 3** -- `xtask/src/sync.rs`: `sync-candidates` runs
  the `git diff --name-status` delta, drops the never-sync
  bookkeeping set, categorizes. Rewrote step 5 of
  `.claude/commands/template-sync.md`.
- **Shared** -- `today_iso` / `is_iso_date` /
  `extract_iso_date` in `xtask/src/helpers.rs`; principle +
  command docs in `CLAUDE.md`.

Note: the three phases share `xtask/src/main.rs` and
`helpers.rs` wiring, so they were validated together; each
phase's runtime behaviour remains independent.

Follow-ups: none required. The ledger format tolerates future
skip/defer keys (parser ignores unknown keys) if that
extension is wanted later.

## Decisions

- **2026-07-16 -- Scope:** build all three phases this
  session, as three independent commits.
- **2026-07-16 -- Watermark storage:** dedicated
  `docs/developer/backfeed-ledger.toml`, one table per
  downstream. It can also record skip/defer decisions and the
  last-seen downstream commit, not just the date.
- **2026-07-16 -- TOML handling:** hand-roll a tiny in-crate
  parser + serializer that regenerates the whole machine-owned
  ledger file on each write. No `toml` crate -- matches the
  `coverage.rs` precedent and avoids the cooldown gate.
