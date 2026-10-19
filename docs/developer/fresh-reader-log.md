# Fresh Reader Findings -- Deferred backlog

Fresh Reader (comprehension) findings that were **deferred** --
real, but not fixed at review time. Most land here because
`/review` stage 3 may not edit clap help, `allowed-tools` or a
rustdoc link. Fixed findings are not logged here; their
resolution lives in the commit that fixed them.

Newest first; add new entries right after the `---`. Use a
self-describing ID `fr-<YYYY-MM-DD>-<kebab-slug>` (no central
counter); a later commit acting on an item cites the ID
inline. Each entry: a `### <id>` heading, a `**Category:**` line,
and a short description.

**Threshold:** when 10+ items are open here, a full-codebase
Fresh Reader review is warranted before continuing feature
work.

---

### fr-2026-10-19-issue-grants-for-review

**Category:** Procedure (`allowed-tools`)

`/issue` calls `/review` through `Skill(review)`, and `/review`
carries no `allowed-tools` on purpose. Neither file says whose
grants apply inside that call. If `/issue`'s apply, its list must
cover every command `/review`'s Snapshot block runs -- it lacks
`mkdir` today. Stage 3 of `/review` may not edit `allowed-tools`,
so this was logged: settle the rule in one sentence and make
`/issue`'s list match it.
