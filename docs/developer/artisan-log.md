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

### aq-2026-07-15-preflight-report-structured-records

**Category:** Type Safety

`Report.pinned` and `Report.dead_ends` in
`xtask/src/dep_age/preflight.rs` store pre-formatted display
strings (`"{name}: {ver} -> {target}"`, `"{name}@{ver}"`)
assembled inside the pure-ish `run` loop. This couples the
loop's unit tests to message wording (they assert on
sentences, not behaviour) and forecloses a non-printing
consumer (a JSON `--format`, per-crate counts) without
re-parsing. Better: store structured records (e.g.
`Pin { name, from, to }`, `DeadEnd { name, version }`) and move
formatting to the `dep_preflight` print site. Deferred to keep
the feature+RT-1-bugfix commit focused; the string form is not
incorrect, only less flexible.

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
