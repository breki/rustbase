# Artisan Findings -- Deferred backlog

Artisan (code-quality) findings that were **deferred** --
real, but not fixed at review time. Fixed findings are not
logged here; their resolution lives in the commit that fixed
them.

Newest first; add new entries right after the `---`. Use a
self-describing ID `aq-<YYYY-MM-DD>-<kebab-slug>` (no central
counter); a later commit acting on an item cites the ID
inline. Each entry: the ID heading, a `**Category:**` line,
and a short description.

**Threshold:** when 10+ items are open here, a full-codebase
Artisan review is warranted before continuing feature work.

---

### aq-2026-07-15-locked-dep-newtype

**Category:** Type Safety

The changed-deps cooldown gate threads `(name, version)` as a
bare `(String, String)` tuple through `parse_cargo_lock`,
`parse_npm_lock`, `new_locked_versions`, and `collect_changes`
in `xtask/src/dep_age.rs`. The two fields are the same type and
positionally interchangeable, so nothing at the type level
prevents a name/version transposition in a future edit. A named
`struct LockedDep { name, version }` would harden the
most-touched data shape. Deferred as low-value: the tuple is
private, module-internal, and its shape is guarded by the
parser/diff unit tests.
