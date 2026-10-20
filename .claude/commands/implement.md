---
description: Plan and implement a captured issue from docs/todo.md
allowed-tools: Bash(cargo xtask*), Bash(cargo -q xtask*), Bash(git status:*), Bash(git diff:*), Bash(git log:*), Bash(git rm:*), Bash(scripts/e2e.sh*), Read, Write, Edit, Glob, Grep, Agent, AskUserQuestion, Skill(commit), Skill(review)
---

Plan and implement an item captured by `/todo`. The
plan lives at `docs/issues/<slug>.md` while the work is
in flight: it is updated as the work progresses (and goes
into any work-in-progress commit you make), and is removed
when the item is done (Phase 3).

## Selecting the issue

- **With a slug argument** (e.g.
  `/implement search-bar-perf`): use that slug. If
  `cargo -q xtask todo list` does not show it, stop and
  tell the user.
- **Without arguments**: run `cargo -q xtask todo list`
  for the pending slugs with their summaries, and ask the
  user which one to implement (use
  `AskUserQuestion`). Do not pick one yourself.

## Phase 1 -- Plan

1. Read `docs/todo.md` and locate the chosen item.

2. If `docs/issues/<slug>.md` already exists, read
   it -- earlier analysis may already be there.
   Otherwise create it.

3. Investigate the codebase enough to write a real
   plan: relevant files, current behaviour, where the
   change lands, risks. Use `Grep`/`Glob`/`Read` or
   delegate broad searches to an `Agent` with
   `subagent_type=Explore`.

4. Write `docs/issues/<slug>.md` with this structure:

   ```
   # <slug>

   **Status:** Planning
   **Captured:** <date the item was added: the oldest
   commit `git log -S '### <slug>' -- docs/todo.md`
   lists, else "unknown">
   **Started:** <today's date>

   ## Problem
   <what the user asked for, in your own words>

   ## Context
   <relevant files, current behaviour, constraints>

   ## Open questions
   <bulleted list -- fill in as you find them>

   ## Plan
   <numbered steps -- concrete, file-level when
   possible>

   ## Test strategy
   <unit tests, E2E tests, edge cases,
   bug-reproduction tests>

   ## Decisions
   <to be filled as questions get answered>
   ```

5. For every open question or design decision that
   materially changes the plan, call
   `AskUserQuestion`. Record the question **and the
   answer** under `## Decisions` in the issue doc.
   One decision per bullet, with date.

6. When the plan is ready, show the user a short
   summary (3-5 bullets max) and ask whether to
   proceed with implementation. Wait for an explicit
   yes. Do not start coding before that.

## Phase 2 -- Implement

Follow the project rules in `CLAUDE.md`. In
particular:

- **TDD applies to behaviour change.** For new logic
  in existing code or a bug fix in shipped code,
  write the failing test first. For structural
  additions (new self-contained module, new helper,
  new enum variant with no callers yet), test and
  implementation may land together as one unit --
  the pre-impl failure step adds no signal there.
  When in doubt, prefer the behaviour-change
  discipline. See `CLAUDE.md` "Test-Driven
  Development" for the full rule.
- **Test level -- prefer the cheapest that proves
  the behaviour.** Rust unit tests for library
  logic, integration tests for CLI behaviour,
  Vitest unit/component tests for frontend logic.
  Reach for a Playwright E2E test only when the
  behaviour genuinely requires a real browser:
  full-page navigation, multi-route flows, real
  network boundaries, focus or keyboard
  interactions that depend on the actual DOM, or a
  bug that only reproduces end-to-end. Note the
  choice briefly in the issue doc's
  `## Test strategy` section. Use the `web-dev`
  skill when writing E2E tests.
- **All tests must pass.** Fix pre-existing failures
  you encounter; do not work around them.
- **Update `Status:`** in `docs/issues/<slug>.md` to
  `In progress` when you start coding. Append a
  `## Progress log` section with short dated entries
  as milestones land.
- **Ask, do not guess.** Use `AskUserQuestion`
  whenever a requirement or trade-off is unclear.
  Record the answer under `## Decisions`.

## Phase 3 -- Finalise

1. Run `cargo -q xtask validate`. Every gate listed
   under **Definition of Done** in `CLAUDE.md` must pass.

2. If the change affects developer workflow or skills,
   update the relevant files under `.claude/commands/`
   and `docs/`. Clean up stale content while you are
   there.

3. **Close out the issue doc.** `docs/issues/<slug>.md`
   is a working document, not a permanent record: it
   mixes reversed intermediate designs with what
   shipped, and its `file:line` references go stale.
   Before removing it, promote what must outlast it:
   - A design or security decision with its rationale,
     or a non-obvious constraint, goes into a reference
     doc under `docs/` (create `docs/<topic>.md` when
     none fits) or into a comment beside the code it
     governs.
   - What was and was not verified goes into the commit
     body (and the PR body, if there is one).

   Then remove the doc: `git rm docs/issues/<slug>.md`
   when it was committed, otherwise delete the file.

4. Remove the item from `docs/todo.md` mechanically (do
   not hand-edit the file):

   ```
   cargo -q xtask todo done <slug>
   ```

   The command removes the entry whole. The queue holds
   live work only; the commit and git history record what
   shipped.

5. Verify the change manually. For UI / API
   changes, exercise the feature in a browser (run
   the backend + frontend dev servers; see
   `CLAUDE.md` "Frontend Development"). For
   deployment-affecting changes consider running
   `cargo xtask deploy` against a staging target.

6. **Review, if you want the work hardened.** Run
   `/review` before committing; it is a separate
   process from `/commit` (see `CLAUDE.md` under
   **Reviewing is its own process**). It comes after
   step 5 on purpose: verification can reshape the
   design, and reviewers should not read code that is
   about to change. Reshaping is likely with inference /
   heuristic logic, open clarifying questions, user data the agent
   has not seen, or an undo / inverse /
   optimistic-concurrency path -- a shape an adversarial
   review routinely overturns, because it looks right
   for the simple case and breaks the recurring or
   concurrent one.

7. Commit with `/commit`.

## Rules

- Never skip the plan phase, even for a small change.
  The issue doc is the working record while the item is
  in flight; step 3 of Phase 3 retires it.
- Never start implementing before the user explicitly
  approves the plan.
- Never remove an item from `docs/todo.md` except
  through `cargo xtask todo done` when finalising. If a
  `todo` command reports the old Pending/Done layout,
  stop and hand the conversion to the operator.
- Use 80-character margins in all Markdown.
