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

### aq-2026-07-16-shared-git-capture-helper

**Category:** Duplication (shared helper)

`deploy_guard::working_tree_clean` and `head_release_tag`
each re-spell the
`Command::new("git").current_dir(root).args(...).output()` +
UTF-8-decode + status-check pattern that
`dep_age::gate::git_show` already implements, and the
"run via `current_dir`, not `git -C`, to stay coverable by a
blanket permission rule" rationale is restated at each site.
`helpers.rs` has cargo runners (`run_cargo_capture`) but no
git equivalent. Consider a shared
`helpers::git_capture(root, args) -> Result<Output, String>`
carrying the rationale once, with `working_tree_clean`,
`head_release_tag`, and `git_show` built on top. Deferred: the
return shapes differ (`Option` vs `Result`,
`from_utf8` vs `from_utf8_lossy`), so it is low-severity
structural spread, not copy-paste; unifying it touches
`gate.rs` outside the current diff.

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
