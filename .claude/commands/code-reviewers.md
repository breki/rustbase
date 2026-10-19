---
description: Code reviewers -- which run and how; the reference /review follows
---

# Code reviewers (which run, and how)

Three reviewers are available to `/review`. Their personas live
as first-class subagents in `.claude/agents/`. This file defines
*which* reviewers run, *when*, and *how* to spawn them; the
review criteria live in the agent files, and the sequence, lanes
and stop rule live in `/review`.

- **`artisan`** (`.claude/agents/artisan.md`) -- code quality &
  craftsmanship beyond clippy. Tools: `Read, Grep, Glob` only.
- **`red-team`** (`.claude/agents/red-team.md`) -- security &
  correctness. Tools: `Read, Grep, Glob, Bash`; it runs
  `git log` itself for the surrounding history.
- **`fresh-reader`** (`.claude/agents/fresh-reader.md`) --
  comprehension. Tools: `Read, Grep, Glob` only. It reads the
  changed files **whole**, not as a diff, because it judges what
  a newcomer lands on rather than what moved.

**Two of them cannot write; the third is asked not to.** None
has `Edit` or `Write`. `artisan` and `fresh-reader` have no
shell, so for those two "read-only" is a harness fact. An
agent's `tools:` field takes tool names, not permission rules,
so `red-team`'s `Bash` is unscoped and only the instruction in
its own file stops it writing. A prompt injected through a file
the repo controls therefore reaches a shell; that risk is
accepted and written down here rather than papered over.

**The three lanes do not overlap.** `red-team` asks whether the
code is safe and correct, `artisan` whether it is well made, and
`fresh-reader` whether a person arriving cold can work out what
it does from the files alone. Both of the first two already know
what the change is for, so neither can notice that the code
never says.

**An edit to an agent file takes effect on the next session,
not this one.** Claude Code reads `.claude/agents/` at startup,
so a new agent cannot be used in the session that created it,
and a round reviewing its own agent files judges them with the
previous version's brief. Say so in the summary when it applies.

## When to run

Judge the change as a whole and sort it into one of two kinds.
`/review` under **Which stages run** turns that into stages.

**Code.** Rust (`.rs`, `.toml`), frontend (`.svelte`, `.js`,
`.ts`, `.css`), config (`playwright.config.ts`,
`vite.config.js`, `vitest.config.js`, ...), scripts (`.sh`,
`.ps1`), workflows under `.github/`, and deployment /
infrastructure files (`.service`, `Dockerfile`,
`docker-compose.yml`, `.conf`, `.nginx`, `.env.example`, ...).
Never skip a review for a "straightforward" change.

**Everything else** -- documentation, `CLAUDE.md` and anything
under `.claude/**`. Review it when the changed files include
something a person lands on: `README.md`, `docs/*.md`, a module
doc, a command or agent file. The reviewers' own backlogs
(`docs/developer/*-log.md`) are exempt; `/review` under
**Snapshot** subtracts them.

## How to spawn

One at a time, never two together:

- `subagent_type: artisan`
- `subagent_type: red-team`
- `subagent_type: fresh-reader`

Give each spawn:

1. A one-line description of what the change does.
2. What **Diff handoff** below says to give that reviewer.
3. Nothing about the report format. Each agent file states its
   own (`AQ-<n>`, `RT-<n>`, `FR-<n>`, each with a
   **Consequence** field); asking for another shape is how a
   report comes back without the IDs `/review` cites.

## Diff handoff

`artisan` and `red-team` are handed the snapshot path `/review`
writes under **Snapshot**, and told the tree may have moved
since. `artisan` has no shell, so say plainly that the path names
a file to read. **Never `/tmp`**: under Git Bash on Windows that
resolves outside the workspace, where the operator cannot see
it. Snapshots live under `target/`.

`fresh-reader` is handed the changed-file list instead and reads
those files as they stand; pointing it at a diff defeats the
point of having it.

Each agent's final message *is* its report (a plain-text finding
list, or "No issues found."), consumed by `/review` -- not
shown to the user directly.

## Reading the reports back

**A reply can arrive with a finding's body missing.** Subagent
replies are occasionally truncated; the tell is an ID that
appears in a summary line but has none of the fields its agent
file specifies. Message that agent again (its ID is in the tool
result) and ask it to re-emit the missing findings verbatim, in
the same structure. Do this before presenting anything: a
dropped finding is one the reviewer did raise and nobody will
notice is gone.

**Two reviewers reaching one defect is the strongest signal
there is.** They count as the same finding when they name the
same `file:line` (or overlapping ranges in one file), or
describe one defect in different vocabulary -- `red-team`'s
"TOCTOU on `is_dir` then `remove_dir_all`" and `artisan`'s
"follows symlinks during deletion despite `dir_size`'s guard"
are one finding about one line. Say which reviewers agreed. A
`fresh-reader` finding pairing with one of the other two means
the defect is real *and* invisible from the files, so fix what
the code says as well as what it does.
