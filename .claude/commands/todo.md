---
description: Process the next pending TODO item
allowed-tools: Bash(cargo xtask*), Bash(git status:*), Bash(git diff:*), Bash(git log:*), Read, Edit, Agent, AskUserQuestion, Skill(commit)
---

Process the next pending item from `TODO.md`.

## Instructions

1. Read `TODO.md` and identify the first pending item
   (items under the `## Done` heading are completed)

2. If the item is ambiguous or has multiple possible
   interpretations, use `AskUserQuestion` to clarify

3. Implement the item following all project rules in
   `CLAUDE.md`

4. Run `cargo xtask validate` to ensure all checks pass

5. Move the completed item to the `## Done` section of
   `TODO.md` with today's date in parentheses

6. Commit using `/commit`
