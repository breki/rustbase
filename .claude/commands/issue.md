---
description: Work a GitHub issue end to end -- verify it is still real, settle the approach, implement with TDD, review, and open a PR that says what was not verified
argument-hint: "<issue number>"
allowed-tools: Bash(gh issue:*), Bash(gh pr:*), Bash(gh run:*), Bash(git status:*), Bash(git diff:*), Bash(git log:*), Bash(git show:*), Bash(git branch:*), Bash(git checkout:*), Bash(git fetch:*), Bash(git pull:*), Bash(git push:*), Bash(git rm:*), Bash(git merge-base:*), Bash(git add -N:*), Bash(git reset --:*), Bash(git ls-files:*), Bash(cargo xtask*), Bash(cargo -q xtask*), Bash(wc:*), Bash(grep:*), Read, Write, Edit, Glob, Grep, Agent, AskUserQuestion, Skill(commit), Skill(review), Skill(todo)
---

Work the GitHub issue given as the argument, from reading it to
reporting back on it.

**Requires the GitHub CLI** (`gh`), authenticated against the
repository's remote. Without it, stop and say so.

Committing happens through `/commit` and reviewing through
`/review`, which every issue gets once the branch is pushed
(step 10). This command reimplements neither.

## Steps

The steps run in the order they are numbered.

### 1. Read the issue, then verify it against the tree

Run `gh issue view <n> --json title,body,state,comments`. Use the
JSON form: the plain `--comments` form can print nothing and
still exit 0, so its empty output cannot be told apart from an
issue with no text.

**Do not trust the issue body.** Somebody wrote it at one point
in time and the tree has moved since. Check every factual claim
before planning any work:

- Does the code it describes still look like that? Open the
  file at the line it names.
- Has the work already landed? `git log --oneline -20`, and
  grep for the symbol or string the issue says is missing.
- Are the numbers still current? Counts, versions and file
  sizes all decay. Re-measure rather than repeating the figure.

If the issue cites a slug in `docs/todo.md` or a planning doc
under `docs/issues/`, read those too: the issue body is a summary
of them, and the summary is what goes stale.

If the issue is already satisfied, close it with the evidence --
file, line, commit -- and say so. Do not do the work again. If it
is only partly satisfied, narrow it: edit the title and body down
to what remains, comment saying what you narrowed and why, and
work the remainder.

### 2. Settle the approach before writing code

If the issue names more than one viable route, or you can see one
it does not name, use `AskUserQuestion`. `CLAUDE.md` under
**Collaboration** asks for the concrete artifacts first and a
recommendation among the options, not an even menu. Read enough
of the code to make the question concrete.

Record the decision. A planning doc under `docs/issues/` holds it
while the work is in flight, but that doc is removed when the
item is done (step 12), so a decision meant to last goes in the
PR body or a reference doc as well.

### 3. Branch

`git checkout -b <type>/<short-name>` from an up-to-date `main`.
Never work an issue directly on `main`.

When another worktree has `main` checked out, `git checkout main`
fails, because git refuses one branch in two worktrees. Branch
from the remote copy instead:

```bash
git fetch
git checkout -b <type>/<short-name> origin/main
git branch --unset-upstream
```

A branch made from `origin/main` takes it as its upstream, so a
plain `git push` would target `main`; unsetting it means the
first push names the branch:
`git push -u origin <type>/<short-name>`.

### 4. Implement, test first

Red then green, per `CLAUDE.md` under **Test-Driven
Development**, including its carve-out for structural additions.
Read it rather than deciding from memory which half you are in.

**A regression test must be proven to fail.** Write it, break
the guard it covers and confirm `cargo xtask test` goes red, then
restore and run it green with `--verbose`. A filter that matches
no test still prints `Test OK`, so only the verbose list shows
the test ran.

**Writing a guard? List its siblings.** A guard protects a
*primitive* -- the file a value is written into, the shell a
value reaches -- not one argument. Name the primitive, then list
every value that reaches it, and guard them all.

**Scope discipline.** Fix what the issue describes. Capture an
adjacent problem with `/todo` rather than folding it in, unless
leaving it would make your own change wrong.

### 5. Keep canon in step

The compiler finds code that references something you changed;
nothing finds the prose. Grep for the old form across
`CLAUDE.md`, `.claude/commands/*.md` (the prose *and* the
`allowed-tools` frontmatter, which is executable),
`.claude/agents/*.md`, `.github/workflows/`, clap `///` help, and
`docs/`. `cargo xtask canon-check` catches a dangling path or
cross-reference in canon, not a stale description. Leave the
historical records alone: dated `CHANGELOG.md` sections and past
backlog entries record what was true then.

### 6. Verify, and know what you did not verify

Run `cargo xtask validate`; `CLAUDE.md` under **Definition of
Done** lists its gates. A UI change also needs the browser check
`validate` cannot do.

Then write down what you could not exercise -- another platform,
an external service, a config you did not break. An unverified
fix is not a failure; presenting it as verified is.

### 7. Self-review the diff

Read your own diff before it becomes a commit. No reviewer
replaces this.

### 8. Commit

Commit through `/commit`. State in the commit body what was
verified and what was not.

### 9. Open the PR at the first push

Open the PR as soon as the first commit is pushed, with
`gh pr create` and `Closes #<n>`, so CI starts on the first push
and the work is visible while it moves. Write the PR body as one
line per paragraph -- GitHub renders the markdown, so a hard
wrap only makes it harder to edit.

Watch that first run in the background, picking it by the pushed
commit's hash rather than "latest" (a new run can take seconds to
appear). Update the title and body whenever the scope changes.

The body carries what a reviewer cannot get from the diff:

- **Why this approach**, and what was rejected
- **What changed**, as a table when the change is structural
- **Deliberate regressions**, named
- **Verification**: the commands run and their results
- **Not done**: what is unexercised, unreviewed or deferred
- **Review**: which round `/review` stopped after, and the
  findings carried forward

### 10. Review the pushed branch

Run `/review` here without asking, and report what it found even
when it found nothing. `/review` owns the stages, the snapshots
and the stopping rule.

**Hand it the branch point.** The work is committed by now, so a
diff against `HEAD` holds nothing:

```bash
git merge-base main HEAD     # pass the result to /review
```

**Its fixes become their own commits.** `/review` edits the tree
and commits nothing, so commit its changes through `/commit`
separately from the implementation -- a bad fix then reverts
without taking the work with it -- push, and update the PR body.

**The PR body lists the findings nobody fixed.** Deferred ones
are logged under `docs/developer/`, which a reader of the branch
will not think to open; declined ones are logged nowhere. Copy
both into the PR body.

### 11. Watch CI, and separate new failures from old

`gh run list --branch <branch>`, then `gh run watch <id>`. A red
step is not automatically yours: compare against `main`, and when
it already failed there, say so rather than fixing it silently
or letting it read as a regression.

### 12. Report back on the issue

Comment on the issue with the outcome: what was fixed, the
commit, the verification, and what remains unverified.

**The operator merges the PR, so this command ends here.** Two
things wait on that merge -- closing the issue and completing its
`docs/todo.md` entry -- so say in your report that both are
outstanding. The one exception is an issue step 1 established
was already satisfied: close that one there and then.

Completing the entry, after the merge:

```
cargo -q xtask todo done <slug> --date <today's date>
```

If a planning doc existed, promote anything durable from it
first -- a design or security decision, a non-obvious constraint
-- into a reference doc under `docs/`, a code comment or the PR
body, then remove it with `git rm docs/issues/<slug>.md`.

## Rules

- Verify before working. The issue is a claim, not a fact.
- Never work an issue on `main`.
- No behaviour change without a failing test first, and a
  regression test nobody has seen fail does not count.
- Always run `/review` on the pushed branch.
- Never report a gate as passing on the strength of a document.
  Run it.
- Say what you did not verify.
