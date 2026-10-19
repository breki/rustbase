---
description: Review the work with three reviewers in sequence, each in one lane -- artisan on the code, red-team on what is unsafe or wrong, fresh-reader on the prose. Commits nothing
argument-hint: "[base commit]"
---

Review the change with one reviewer at a time, fixing what each
finds before the next one reads. **This command never commits.**
It leaves the tree edited, reports what happened, and the
developer decides when the work becomes a commit. Nothing
requires this command; `CLAUDE.md` under **Reviewing is its own
process** says why.

This file carries no `allowed-tools` line on purpose. An
incomplete list is worse than none: a step that runs something
the frontmatter does not grant simply fails at that step.

## What the reviewers read

The argument is the commit the working tree is compared
against, and it defaults to `HEAD`. With the default the
reviewers read what is not committed yet.

Pass a base when the work is already committed -- for a feature
branch, `$(git merge-base origin/main HEAD)` after a `git fetch`
(local `main` can lag, e.g. when another worktree holds it), so
the diff holds every commit on the branch plus anything still
uncommitted, rather than the empty diff `HEAD` gives once the
work is committed.

Call the value `BASE` below.

## Which stages run

**A change containing source code runs all three stages.**
Source code is what `code-reviewers.md` under **When to run**
lists as code. Such a change usually carries documents and canon
with it, and those are reviewed here too.

**A change with no source code in it runs stage 3 alone.**
`artisan`'s and `red-team`'s lanes are defined for code, and
stage 2's stop rule needs a behaviour defect to turn on, which a
prose-only change cannot produce. `cargo xtask canon-check`
covers part of what `red-team` would have caught there (a path
or cross-reference that does not resolve); the rest is a gap we
accept.

## What this file owns

`code-reviewers.md` owns the three reviewers, what each is
handed and how to spawn them. This file owns the sequence, the
lanes, the fixing rules and the stop rule.

**Each stage narrows its reviewer to one lane**, and a lane is
a kind of defect rather than a kind of file:

| Stage | Reviewer | Lane |
|-|-|-|
| 1 | `artisan` | is the source code well made |
| 2 | `red-team` | is it unsafe, or is it wrong |
| 3 | `fresh-reader` | can somebody new follow it |

Nothing outside a stage's lane is fixed in that stage. A
reviewer will report outside it anyway -- tell it to say so in
one line, and carry the item to the stage that owns it rather
than dropping it.

**Why one at a time.** Reviewers spawned together report one
defect three times and see it fixed once, and each reports
against a tree the others' fixes then move. In sequence each
reviewer reads the previous one's fixes, so a bad fix is caught
inside the run rather than a round later.

**Why this order.** Each stage's fixes are less dangerous than
the one before. `artisan` changes types, tests and signatures;
`red-team` changes logic; `fresh-reader` changes only prose. So
the stage that finds danger runs after the stage that creates
most of it, and the stage that cannot break the program runs
last.

## Before any stage

### Snapshot

Reviewers must read one immutable text, not a tree that moves
while they read it.

```bash
mkdir -p target
BASE=HEAD                            # uncommitted work
# BASE=$(git merge-base origin/main HEAD)  # feature branch; fetch first
git ls-files --others --exclude-standard  # untracked: check first
git add -N <the untracked paths of this work>
EXCL=':(exclude)docs/developer/*-log.md'
OUT=target/review-1-1                 # review-<stage>-<round>
git diff "$BASE" -- . "$EXCL" > "$OUT.diff"
git diff --name-only --diff-filter=d "$BASE" -- . "$EXCL" \
  > "$OUT.files"
```

That pair is stage 1's snapshot. Every later one is the same
block with `OUT` advanced to `target/review-<stage>-<round>`,
all with the same `BASE`, so no snapshot overwrites another.

- `git add -N` records a path in the index without its
  contents, which is what makes `git diff` report an untracked
  file at all. Name the paths: `git add -N .` sweeps in the
  developer's scratch notes, which then reach the reviewers and
  land in the next `git commit -a`.
- `:(exclude)<pattern>` subtracts matches from the paths before
  it, which is why `.` comes first. The backlogs are subtracted
  because the stages write to them, and left in they would hand
  each round the previous round's own report.
- `--diff-filter=d` (lowercase: exclude) drops deleted paths,
  which `fresh-reader` can only fail to open.
- The intent-to-add entries stay in the index. Report them with
  their undo, `git reset -- <the paths added with -N>`: until
  then any `git commit -a` commits those paths, including ones
  the run concluded should not ship.

### Run it before anyone reads it

An **artifact** is one runnable thing the change produced or
touched: a build, a sample config, a quoted command, a document
somebody can follow. Reading cannot find "this step assumes a
file no earlier step created"; running it can.

Record every artifact as **run**, **could not run**, or **must
not run**. A run that fails is a finding, treated like a
reviewer's.

- **A code change** -- `cargo xtask validate --check`, so the
  formatter cannot rewrite the tree the snapshot just captured.
  A UI change also needs the browser check from **Definition
  of Done**.
- **A sample config, a quoted command** -- execute it and
  compare. Do not eyeball it.
- **A procedure document** -- follow it from the state it
  names, unless following it mutates something you do not own
  (a shared host, production); then it is *must not run*.
- **A workflow file** (`.claude/**`, `CLAUDE.md`) -- walk it
  against the current tree yourself. An edited agent file only
  takes effect next session, so record that part as *could not
  run*.

**If you fix anything while running the artifacts, run the
whole Snapshot block again before spawning anyone**, from
`git ls-files --others` down: a fix that adds a file changes
the `.files` list as well as the diff.

### Re-snapshot before each stage and each round

A stage reads the previous stage's fixes, so a stale snapshot
hides exactly what the sequence exists to catch.

### Each stage writes its findings

Write them to `target/review-<stage>-<round>.findings` before the
next spawn, one line per finding: its ID, the reviewer, the
category, the `file:line`, one sentence of what it said, and its
disposition. Record the finding, not just its label: the next
stage is asked whether a new finding is a defect in an earlier
fix, and `AQ-7 | fixed` cannot answer that. A file on disk also
survives a long session's summarised context. `target/` is not
committed, so anything that must outlive the run goes in a
backlog under **Log what you defer**.

## Fixing what a stage finds

Check the replies first -- `code-reviewers.md` under **Reading
the reports back** covers a truncated reply and two reviewers
reaching one defect.

**Count the copies first.** On a finding of the shape "X is
wrong in F", find every place that says X before touching one.
Fixing three of four copies earns a round that finds the
fourth.

**Two prose copies are allowed; a third is the defect.** A copy
whose job is helping a reader *find* the rule -- a frontmatter
`description`, a skills-table row, a clap `///` line -- does not
count, though it must still agree with the rule. Above two, the
repair is one authoritative statement plus pointers, and that
repair is **escalated, never applied in the round that found
it**: collapsing copies creates pointers, and each pointer is
new text the next round reviews. Name the copies and hand the
developer a consolidation to make as its own commit.

**Enumerate before you claim a set is done.** Read the list
back and count it.

**Do not fix everything.** Every edit is new text for the next
reviewer. Apply the mechanical ones directly -- a stale doc, a
tightened regex, a renamed local -- and announce the set so the
developer can interrupt. Fix what is wrong or false, and what
would make someone act on it wrongly -- a reader, the operator,
or the program itself, which each reviewer's **Consequence**
field names. Leave what would merely read better.

**Escalate rather than apply** a finding that crosses one of
these: large rework (over five files, over a hundred lines, or
churn outside the diff); two findings conflicting; a genuine
design tradeoff; a public-surface or breaking change; a new
dependency; a consolidation of three or more copies; a fix
that introduces a new parsing heuristic (it is a design choice,
and its edge cases are the next round's findings); out of scope
for the work in hand. Present it in the fields its
reviewer emitted and ask via `AskUserQuestion`: fix it now,
defer it, decline it, or leave it for the developer to decide
before committing the review's edits (the work under review may
already be committed).

**Read the artifact back before claiming a fix landed.** "The
help now says Y" needs the grep that shows it.

### Log what you defer

A fixed finding gets no entry; its resolution lives in the
eventual commit message. Only a deferred one is logged, in
`docs/developer/redteam-log.md`, `artisan-log.md` or
`fresh-reader-log.md` by reviewer. All three are newest-first,
new entries right after the `---`. The ID is
`<rt|aq|fr>-<YYYY-MM-DD>-<kebab-slug>`, so there is no counter
to keep and the ID greps. Each entry is a `### <id>` heading
(the level `canon-check` reads), a `**Category:**` line, and a
short description.

**Closing one is the other half of the rule:**

- A **fixed** item leaves no entry.
- A **standing decision** ("keep this, and here is what would
  change it") does not belong in a deferred backlog: move it
  into a comment beside the code it governs, with its revisit
  condition, and delete the entry.
- **Before deleting any entry, run `grep -rn "<id>" .`** -- the
  IDs are greppable so other documents can cite them, and a
  delete can strand a pointer. `cargo xtask canon-check` fails
  on a dangling ID in canon and `docs/`.

Before fixing a logged entry, reproduce it against the current
tree; a later change may have closed part of it. When ten or
more sit open in one backlog, say so: a full-codebase review is
warranted.

## Stage 1 -- `artisan`, once

Spawn `artisan` alone and fix what it finds.

**Ask it for the source code only.** Its error-handling, API
design, abstraction, type-safety and module-size categories are
this stage. Its `Canon and documentation` category is **not**:
tell it that documentation belongs to stages 2 and 3, so a
finding it has there arrives as one line.

One stage, not a loop. `red-team` reads these fixes next, which
is what a second artisan round would otherwise be for.

## Stage 2 -- `red-team`, until behaviour settles

Each round: re-snapshot, spawn `red-team` alone, fix, write the
findings.

**Hand over every earlier stage's and round's findings** and
ask outright: *is anything here a defect in the fix for an
earlier finding?* A reviewer shown only the current state
cannot see a loop.

**Ask it whether the change is unsafe or wrong.** Wrong
includes prose that states a false rule or fact: a document
claiming what the code does not do, a step nobody can follow, a
CHANGELOG recording half a change. Prose that is merely unclear
or badly placed is stage 3's, and goes there as one line.

**A finding whose fix changes behaviour gets its failing test
first**, per `CLAUDE.md` under **Test-Driven Development**. A
green suite does not mean this stage has nothing to find: a test
asserts the property somebody thought of, and this stage exists
to find the one nobody did. A finding whose fix changes only
prose gets no test.

### What earns another round

A **behaviour defect** is a finding whose fix changes what the
program does: what it prints, what commands it emits, what
input it accepts or refuses, what it writes or deletes, or what
a caller can compile against. A finding about a comment, a
document, a test, a name or a record is not one, however right
it is.

**Another round is earned only when the round found a behaviour
defect and we fixed it.** So:

- A round returning only comment, document and test findings
  ends the stage. Fix them, then go to stage 3.
- A round whose behaviour defects were all **deferred or
  declined** also ends it: the tree did not move, so the next
  round would read the same code.

The two conditions tend to fire together: a stage whose findings
are all prose is also one whose fixes are all prose, and
rewritten prose is where the next round's findings come from.

**Stop early when earlier fixes are breaking** -- more than one
defect in an earlier round's fix, or one landing where an
earlier round already fixed something -- and go to **When it
stops converging**. A single isolated defect in a fix is not
that: fix it, note it, and carry the count forward.

**Three rounds is the ceiling.** A flat tail of findings is not
convergence; reaching the ceiling means the rule above failed,
so say so rather than reporting the run as finished.

## Stage 3 -- `fresh-reader`, prose only

Re-snapshot after stage 2's last fix and hand it the
changed-file list rather than a diff -- `code-reviewers.md`
under **Diff handoff** says why -- with every earlier stage's
findings, the one-line hand-offs included.

**Its lane is whether somebody new can follow the files**:
an explanation missing, a term never introduced, an order that
makes the reader work backwards, a comment narrating history
instead of giving the reason.

**Its fixes touch prose that only a person reads**: code
comments, doc comments, files under `docs/`, `README.md`, and
the prose of `CLAUDE.md` and `.claude/**`. Three kinds of prose
are outside that, because the program
reads them too:

- **clap `///` help** -- it is what `--help` prints.
- **`allowed-tools` frontmatter in `.claude/`** -- the harness
  executes it.
- **A doc-comment edit adding, removing or retargeting a
  rustdoc link** -- `cargo xtask doc` fails on a link that does
  not resolve.

A finding against one of those is real and gets **logged, not
fixed**, per **Log what you defer**.

**Stage 3 runs once.** Its fixes are prose-only, so by the
stage-2 rule they earn no further round.

## When it stops converging

Findings landing on earlier fixes say something about the work,
not that the fixes were careless. Stop fixing and tell the
developer: name the file or fact, give the chain (round N said
X, the fix did Y, round N+1 found Z), and say which of these it
looks like:

- **The rule has no single home.** One rule in several places,
  none saying which copy wins. Commonest and cheapest.
- **The assertion has no contract behind it.** A test or
  document checks something with no stable shape -- rendered
  output, prose, a number that is really a file's length.
  Usually delete it rather than parse harder.
- **Two owners decide, and neither states the invariant.**
- **Only running it finds the gaps** -- for artifacts recorded
  as *could not run* or *must not run*.
- **The work fights the design.** Every fix is correct and it
  still does not settle.

Give at least two options with the trade-off named, say which
you would pick and why, and use `AskUserQuestion` when the
choice changes what happens next.

## Reporting

Per stage: findings raised, fixed, logged, escalated -- and for
stage 2 the round count and why it stopped, naming the behaviour
defect that earned the last round or saying none did.

Then the closing account: every artifact with its label and,
where it failed, what failed; each non-converging area in the
shape above; and what the run left behind, because the
developer's next commit sees all of it -- the edited files, the
backlog files written, and the intent-to-add entries from the
snapshot. Nothing was committed.

## Rules

- **Never commit, never amend, never push.**
- One reviewer at a time. Spawning two defeats the point.
- Each stage asks for one lane. `artisan` is never asked about
  documentation.
- A finding outside the current stage's lane is carried to the
  stage that owns it, never dropped and never fixed early.
- A change with no source code in it runs stage 3 alone.
- Re-snapshot before every stage and round, and write each
  stage's findings before the next spawn.
- Never skip the artifact run because the change obviously
  works.
- Never correct one of several copies without counting them.
- A behaviour defect that was deferred does not earn another
  round.
- `fresh-reader` never edits clap help, `allowed-tools`, or a
  rustdoc link.
- Surface every finding, whatever its disposition. Never drop
  one silently.
- If a finding is wrong, say so with the evidence and move on.
