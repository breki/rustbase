# Red Team Findings -- Deferred backlog

Red Team findings that were **deferred** -- real, but not
fixed at review time. Fixed findings are not logged here;
their resolution lives in the commit that fixed them.

Newest first; add new entries right after the `---`. Use a
self-describing ID `rt-<YYYY-MM-DD>-<kebab-slug>` (no central
counter); a later commit acting on an item cites the ID
inline. Each entry: the ID heading, a `**Category:**` line,
and a short description.

**Threshold:** when 10+ items are open here, a full-codebase
red team review is warranted before continuing feature work.

---

### rt-2026-07-16-deploy-guard-toctou

**Category:** Security (TOCTOU)

`deploy_guard::require_release_tag` checks
`working_tree_clean` at the top of `deploy()`, but the tree
is not re-read until `sync_source` tars the sources and
`build_frontend` builds moments later. A change to the tree
in that window ships un-verified content under a tag that
vouches for the clean state. Low severity: operator-only
threat model (self-inflicted race, not an adversary), and the
binary is rebuilt from the tarred sources, not a stale
artifact. If tightening is wanted, re-assert
`working_tree_clean` immediately before `sync_source`, or tar
from `git archive <tag>` so the shipped source is provably the
tagged commit.
