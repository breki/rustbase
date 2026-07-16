# changelog-version-drift

**Status:** Done
**Captured:** 2026-07-16
**Started:** 2026-07-16
**Completed:** 2026-07-16

## Problem

`CHANGELOG.md`'s newest dated release is `[0.10.0]`
(2026-05-18), but `crates/rustbase/Cargo.toml` is at 0.15.0.
Nine version bumps (0.10.1 through 0.15.0) landed under the
old per-commit `/commit` model without ever getting dated
CHANGELOG sections -- their changes are pooled in the ~250-line
`[Unreleased]` block. No git tags exist for any version. The
new `/release` model only promotes `[Unreleased]` to the
*next* version, so the gap would persist and grow.

## Context

- `CHANGELOG.md`: dated sections stop at `## [0.10.0] -
  2026-05-18` (line ~261). `[Unreleased]` (lines 11-260)
  holds Added / Changed / Fixed / Removed entries spanning all
  nine post-0.10.0 bumps.
- Each version maps cleanly to exactly one commit (verified via
  `git show <c>:crates/rustbase/Cargo.toml`):

  | version | date | commit | subject |
  |---|---|---|---|
  | 0.10.1 | 2026-05-18 | d29e057 | hoard backfeed (junction guard, release-fast, stop-hook) |
  | 0.10.2 | 2026-07-13 | 60660da | jutro stage 1 (tooling safety & diagnostics) |
  | 0.10.3 | 2026-07-13 | 1bca6f9 | ESLint parses TS in Svelte |
  | 0.10.4 | 2026-07-14 | bb363c3 | jutro stage 2 (xtask diagnostics & validate ergonomics) |
  | 0.11.0 | 2026-07-14 | fe79a8d | jutro stage 4 frontend (version header + Prettier/jscpd/vitest gates) |
  | 0.12.0 | 2026-07-14 | 4602285 | jutro stage 4 supply-chain (audit gate + dep-age) |
  | 0.13.0 | 2026-07-15 | 6038f1d | dependency-cooldown validate gate |
  | 0.14.0 | 2026-07-15 | 44561fd | /update-deps + dep-age --latest-aged |
  | 0.15.0 | 2026-07-15 | 720f090 | dep-preflight |

- The `[Unreleased]` bullets are feature-specific and the
  commit subjects are descriptive, so mapping each bullet to
  the version that introduced it is tractable (dep-preflight ->
  0.15.0, /update-deps -> 0.14.0, cooldown gate -> 0.13.0,
  audit/dep-age -> 0.12.0, frontend gates -> 0.11.0, the four
  fixes -> 0.10.1-0.10.4).
- This is a template repo: downstreams consume specific commits
  via `/template-sync`, so per-version history has real value
  for "what changed when".
- Docs-only change; no Rust, no `validate` gate impact beyond
  markdown wrapping.

## Open questions

- Backfill full dated sections per version, or add a single
  consolidated note explaining the gap? (See Decisions.)

## Plan

(Finalised after the backfill-vs-note decision below.)

Backfill path:
1. Partition the `[Unreleased]` entries by the version/commit
   that introduced each, using the mapping table above.
2. Create dated `## [X.Y.Z] - YYYY-MM-DD` sections (0.15.0
   down to 0.10.1) in descending order, above `[0.10.0]`, each
   grouped by Added / Changed / Fixed / Removed.
3. Leave `[Unreleased]` with only genuinely-unreleased items
   (the `/release`/`/html-report`/deploy-guard work from the
   two most recent commits, which are not yet under a bumped
   version -- they will be cut by the first `/release`).
4. Wrap all markdown at 80 chars.

## Test strategy

No automated test -- this is a hand-authored markdown history
reconstruction. Verification: visually diff the moved entries
to confirm none are dropped or duplicated, and confirm the
`## [X.Y.Z]` headers parse under the Keep-a-Changelog format
the release workflow expects.

## Progress log

- 2026-07-16: Derived the deterministic entry->version map via
  `git show <commit> -- CHANGELOG.md` for every
  CHANGELOG-touching commit since v0.10.0 (including the chore
  commits that added entries without bumping). Partitioned the
  36 `[Unreleased]` bullets into dated sections 0.10.1-0.15.0,
  keeping the 4 genuinely-unreleased entries
  (`/release`, `/html-report`, `/commit`-split, deploy-guard)
  in `[Unreleased]`. Verified post-edit: 36 bullets preserved
  (none lost/duplicated), `[Unreleased]` has 4, headers in
  descending order, and the pre-existing duplicate
  `### Changed` heading inside `[Unreleased]` is gone.

## Outcome

Backfilled dated CHANGELOG sections for every version between
0.10.0 and 0.15.0 (`CHANGELOG.md` lines ~45-316): 0.15.0,
0.14.0, 0.13.0, 0.12.0, 0.11.0, 0.10.4, 0.10.3, 0.10.2,
0.10.1. Each entry sits under the version whose commit
introduced it, grouped Added / Changed / Fixed. `[Unreleased]`
now holds only the four entries committed under the new
save-point `/commit` model (no version bump), which the first
`/release` will cut. No git tags were created (out of scope --
the versions were never tagged; tagging retroactively is a
separate call if ever wanted). Docs-only change; no code or
`validate` impact.

## Decisions

- 2026-07-16: **Backfill full dated sections** (not a
  consolidated note). Each version maps cleanly to one
  descriptive commit and this is a template downstreams sync
  from per-commit, so accurate per-version history is worth the
  effort. The `[Unreleased]` block is partitioned by the
  version/commit that introduced each entry; the
  `/release`/`/html-report`/deploy-guard work (committed
  without a bump under the new model) stays in `[Unreleased]`
  for the first `/release` to cut.
