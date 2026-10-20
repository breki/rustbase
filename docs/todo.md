# TODO

Project work queue.

- `/todo <text>` captures a new item with a generated slug.
- `/todo` (no arguments) lists pending slugs.
- `/implement <slug>` plans and implements a pending item.
- `/implement` (no arguments) lists pending items and asks
  which to act on.

While an item is being implemented it gets a working planning
doc at `docs/issues/<slug>.md` (problem, plan, decisions). It is
removed when the item lands, with any durable decision promoted
into the reference docs first -- see the `/implement` skill.

Each item is a headed entry: an `### <slug>` heading, a
`**Summary:**` line, then any prose body. Add and complete items
through `cargo xtask todo`; completing one removes it, and git
history records what shipped. The file is this introduction, then
items only: `list`, `add` and `done` refuse a non-item heading after the
first item. Do not hand-edit it: `todo add` refuses a body line that
would read as a heading, and a hand edit can slip one in.

### xtask-strip-web

**Summary:** Ship `cargo xtask strip-web` to convert the template to CLI-only

A one-shot in-place mutation that converts the template into a CLI-only project.
Touches: `crates/rustbase-web/` (delete), `frontend/` (delete), `Cargo.toml`
workspace members (remove web crate), `.github/workflows/` (remove frontend
job), `build.ps1` (remove Invoke-Dev/Invoke-Frontend/Invoke-E2E functions and
dispatch branches), `README.md` + `llms.txt` (drop web-app sections),
`CLAUDE.md` (drop Frontend Development + E2E Testing sections), `scripts/e2e.sh`
+ `scripts/kill-servers.sh` (delete, orphaned without frontend),
`.ports.sample`, and `playwright.config.ts` + root `tsconfig.json`. Requires a
git-clean precondition check (irreversible in-place edit). Should land with its
own xtask tests exercising the file-mutation logic against a fixture workspace.
Significant scope -- worth a focused session and a
`docs/issues/xtask-strip-web.md` planning doc up front. Sourced from kozmotic's
template feedback (2026-05-04).

### coverage-excluded-lines

**Summary:** Report lines excluded by the coverage regex

From bombyx template feedback (2026-08-18,
tf-2026-08-18-coverage-gate-cannot-see-src-bin). cargo xtask coverage prints
e.g. "98.0%" but never says the IGNORE_REGEX hid all of src/bin/ from the
denominator, so the number reads as a statement about the whole project. Have
coverage report what was excluded -- a file/line count beside the percentage,
e.g. "98.0% of 2,102 lines; 640 lines in 1 file excluded". Design decision:
computing the excluded count needs either a second llvm-cov pass without the
ignore filter (doubles the slowest gate) or counting the excluded files
line-by-line; the per-file floor has the same blind spot. Not a mechanical port
-- bombyx only suggested it, never implemented it.

### drop-npm-cooldown-pins

**Summary:** Remove the npm cooldown pins after 2026-11-01

frontend/package.json pins vitest exactly at 4.1.11 and overrides postcss
(8.5.29) and undici (7.30.0), because the newer releases were inside the 14-day
cooldown on 2026-10-19 (vitest 4.1.12 and undici 7.30.1 clear on 2026-10-29,
postcss 8.5.31 on 2026-11-01). Left in place they freeze those packages. After
2026-11-01: restore the vitest caret range, delete the overrides block, run
/update-deps for the frontend, and confirm npm audit and dep-age-check stay
clean.

### cli-only-recipe-deploy

**Summary:** CLI-only recipe leaves deploy pointing at the deleted rustbase-web

The CLAUDE.md CLI-only recipe ("Project Overview", steps 1-7) never handles
deploy: `cargo xtask deploy` builds and ships `rustbase-web`, which step 5
deletes, so after the recipe `deploy`, `deploy-setup`, build.ps1's deploy
targets, `deploy/rustbase-web.service`, `docs/deployment.md`, and the prose that
tells the operator to run `cargo xtask deploy` (CLAUDE.md "Commits and
releases", .claude/commands/release.md, commit.md, implement.md, README.md) are
all left pointing at a web server that no longer exists. Extend the recipe: drop
`deploy` from xtask's default features, prune build.ps1's deploy targets, delete
deploy/ and docs/deployment.md, add `deploy` to the step-6 sweep names. Found by
red-team (RT-1) while reviewing issue #5.

### todo-edit-subcommand

**Summary:** Add `cargo xtask todo edit` to revise a captured entry

The `/todo` rules forbid hand-editing `docs/todo.md`, but `todo` offers only
list/add/done, so a correction to a captured item forces a hand edit or a
duplicate slug describing the same work. Add `todo edit --slug <slug>` that
rewrites the summary, the body, or both through the existing wrap-and-render
path, so a revised entry is indistinguishable from a fresh one; the reader
already tolerates a multi-paragraph body, and the writer must preserve that.
Failing that, soften the rule in `.claude/commands/todo.md` to name the edit the
tooling cannot make. Sourced from bombyx template feedback (2026-08-30).
