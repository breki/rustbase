---
description: Commit current changes following project conventions
allowed-tools: Bash(git status:*), Bash(git diff:*), Bash(git log:*), Bash(git tag --list:*), Bash(git add:*), Bash(git commit:*), Bash(scripts/e2e.sh*), Bash(cargo xtask*), Bash(cargo -q xtask*), Read, Edit, AskUserQuestion, Skill(retrospect)
---

Commit the current changes following the project's git commit
conventions.

`/commit` is a save-point, not a release event. It does **not**
bump the version, sync `Cargo.lock`, or run
`cargo xtask validate`. Those belong to `/release`, which is
invoked separately (and is required before
`cargo xtask deploy`). `/commit` itself never runs
`cargo xtask validate`; if you want the full gate on a
work-in-progress, run it manually at your own shell (outside
this flow). `/release` runs it as the authoritative release
gate.

`/commit` does no reviewing either. Run `/review` first when
you want the work hardened; `CLAUDE.md` under **Reviewing is
its own process** says why the two are separate.

## Instructions

1. **Analyze current state** - Run these commands in parallel:
   - `git status` (never use -uall flag)
   - `git diff` for unstaged changes
   - `git diff --cached` for staged changes
   - `git log --oneline -5` for recent commit style reference

2. **Review changes** - Analyze what was changed and determine:
   - The commit type: feat, fix, chore, refactor, docs, test,
     style, perf
   - A concise subject line (imperative mood, no period)
   - A brief body explaining what and why

3. **Update CHANGELOG.md** (for user-observable
   changes):
   - The trigger is the **observable effect**, not the
     commit type. If a user of the software would see
     a difference (new feature, fixed bug, changed
     default, removed flag, new config knob, port
     change, new env var, ...), add a bullet to the
     `[Unreleased]` section -- **even if the commit
     type is `chore`** (e.g., a `chore:` that changes a
     default port still needs a `Changed` entry).
   - Add it mechanically rather than hand-editing (the
     `[Unreleased]` subsections can sit dozens of lines
     apart, so a hand edit easily splits a block with a
     duplicate heading):

     ```
     cargo -q xtask changelog add --kind <added|changed|fixed|removed> \
       [--breaking] "<entry text>"
     ```

     The command finds the right `### <kind>` heading
     under `[Unreleased]` (creating it in canonical
     order only if absent) and wraps the text to 80
     columns. `--breaking` prefixes `**BREAKING:**` so
     `/release` infers a major bump from the accumulated
     `[Unreleased]` entries.
   - **Confirm the behaviour shipped before feeding the
     major-bump inference.** `--breaking` *and*
     `--kind removed` both make `/release` infer a major
     bump, so a change that removes or breaks behaviour
     which **never shipped in a release** would force a
     spurious major bump to protect users who never had
     it. This bites hardest pre-1.0 (every fresh scaffold):
     check `git tag --list --sort=-v:refname` -- no tags
     means nothing has ever been released; otherwise the
     first line is the latest tag (a bare git tag listing sorts
     alphabetically, putting `v0.9.0` after `v0.11.0`) --
     and use `git log <latest-tag>..HEAD` or the `[Unreleased]`
     block, since behaviour introduced since the last
     release never shipped either. When it never shipped,
     do not mark it breaking and do not file it under
     `### Removed`; instead **correct the existing
     `[Unreleased]` bullets** that describe the old
     design. That correction is the one sanctioned hand
     edit of `[Unreleased]` (the `changelog` command only
     appends) -- after it, re-check for a split subsection
     or a duplicated `### <kind>` heading.
   - Skip only for commits with no user-observable
     effect: pure refactors, internal tooling, test-
     only changes, CI/lint config tweaks invisible to
     users, docs-only edits.

4. **E2E tests** -- Run `scripts/e2e.sh` to verify the
   full stack works end-to-end. The script kills stale
   servers and runs Playwright, which auto-starts both
   backend and frontend using test data (not production
   data).
   - If E2E tests **fail**, ask the user whether to
     commit anyway or abort.
   - Skip if no frontend or API changes in the diff.

5. **Fix line endings** - After staging, check for CRLF
   warnings. All text files must use LF line endings.

6. **Stage files** - Add specific files by name (avoid
   `git add -A` or `git add .`). Never commit sensitive
   files (.env, credentials, etc.). Include the
   changelog if updated.

7. **Commit** using this exact format (use HEREDOC):

```bash
git commit -m "$(cat <<'EOF'
<type>: <subject>

<body>

AI-Generated: Claude Code (<ModelName> <YYYY-MM-DD>)
EOF
)"
```

8. **Workflow retrospective** -- delegate to
    `/retrospect` (runs *after* the commit lands so
    it cannot block shipping).

    The `/retrospect` skill owns the full set of
    rules: the four buckets (Efficiency / Quality /
    Speed / Cleanup), `[trivial]` vs `[propose]` tagging,
    the offer to auto-apply trivial findings, and
    the recursive-skip carve-out for workflow-only
    diffs (`.claude/**` / `CLAUDE.md` only). See
    `.claude/commands/retrospect.md` for the full
    contract.

    From here, simply invoke `/retrospect`. If the
    just-committed diff would trigger the recursive
    skip, `/retrospect` no-ops silently. Otherwise
    it produces the report inline.

## Rules

- DO NOT include "Co-Authored-By" lines
- DO NOT include "Generated with [Claude Code]" lines
- Use the AI-Generated footer format shown above
- If no changes to commit, inform the user
- If changes look incomplete or risky, ask before committing
- Never bump `crates/rustbase/Cargo.toml` from `/commit`.
  That is `/release`'s job.

## Commit Types

The commit type no longer drives a version bump directly.
`/release` computes the bump from the accumulated
`[Unreleased]` CHANGELOG entries since the last release --
the "eventually" below refers to that later `/release`.

- `feat`: New feature (eventually a minor bump at release)
- `fix`: Bug fix (eventually a patch bump at release)
- `perf`: Performance improvement (eventually a patch bump)
- `chore`: Maintenance, tooling, dependencies
- `refactor`: Code restructuring
- `docs`: Documentation only
- `test`: Adding or updating tests
- `style`: Formatting, whitespace
