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

### fr-2026-10-19-audit-result-warnings-name

**Category:** naming

In `crates/rustbase-xtask/src/audit.rs`, `AuditResult.warnings`
holds the reasons a check did not finish, which can fail the gate
under `Strictness::Fail`; `CargoAudit.warnings` counts cargo-audit's
harmless advisory warnings. Same word, opposite weight. `verdict`
already calls the first `unfinished`; rename the field to match.
Deferred because stage 3 does not rename code.

### fr-2026-10-19-validate-order-rationale-copies

**Category:** Duplication (one rule, four homes)

Why `validate` runs its steps in their order is written four times:
the `validate()` doc in `xtask/src/validate.rs`, the comments in
`steps()` (which defer to it), the ordering paragraph after the
Definition of Done list in `CLAUDE.md`, and per gate in each
`step()` doc (Dep-age's network behaviour in
`xtask/src/dep_age_gate.rs`). They had drifted: the `validate()`
doc said Dep-age "never blocks", which is false under CI. That
clause was fixed in place for issue #5. Consolidation is left to
its own commit: state the order once, on `steps()`, cut the
`validate()` doc to what it does, and leave each gate's network
behaviour to its own `step()` doc.

### fr-2026-10-19-history-comments-in-xtask

**Category:** Comments (history and round-local IDs)

Several xtask comments narrate history or cite review IDs that no
backlog holds, so a reader has nothing to follow: `feedback.rs`
("`/template-improve` used to hand-edit the feedback file"),
`sync.rs` (the module doc's "already SHA-delta based, but ..."),
`audit.rs` ("this is the RT-1 fix"), `coverage.rs` ("(RT-2 /
AQ-2)", "(RT-3)") and `deny.rs` ("that shape let the distinction
live in a string comparison"). Restate each as the present-tense
reason and drop the round-local IDs. These predate issue #5,
whose review found them.

### fr-2026-10-19-issue-grants-for-review

**Category:** Procedure (`allowed-tools`)

`/issue` calls `/review` through `Skill(review)`, and `/review`
carries no `allowed-tools` on purpose. Neither file says whose
grants apply inside that call. If `/issue`'s apply, its list must
cover every command `/review`'s Snapshot block runs -- it lacks
`mkdir` today. Stage 3 of `/review` may not edit `allowed-tools`,
so this was logged: settle the rule in one sentence and make
`/issue`'s list match it.
