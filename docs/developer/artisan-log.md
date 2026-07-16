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

### aq-2026-07-16-consolidate-date-helpers

**Category:** Duplication (module cohesion)

Date math is split across two modules with duplicated
primitives. `dep_age.rs` privately owns `days_from_civil`,
`parse_iso_date`, `age_in_days`, and `today_days`; `helpers.rs`
now adds `civil_from_days` (the inverse of `days_from_civil`),
`is_iso_date`, `extract_iso_date`, and `today_iso`. The
epoch-days snippet in `today_iso` duplicates `today_days`, and
the two civil-date converters are inverses living in different
files. Consider consolidating all civil-date / ISO helpers
into one home (a `xtask/src/dates.rs`, or `helpers.rs`) and
having `dep_age.rs` call the shared versions. Deferred:
unifying touches `dep_age.rs` and its private tests, both
outside the current diff, and the duplication gate is at 0%
today so there is no forcing pressure. Same
"consolidate the hand-scan primitives" theme as
`aq-2026-07-16-shared-toml-scan-helper`.

### aq-2026-07-16-split-backfeed-module

**Category:** Module size

`xtask/src/backfeed.rs` (~360 production lines, test-heavy)
mixes two concerns: ledger TOML parse/serialize (`Ledger`,
`parse_ledger`, `serialize_ledger`, `unquote`) and feedback
-markdown scanning (`hash_level`, `boundary_levels`,
`entry_blocks`, `entries_in_scope`). If it grows further,
split the ledger half into a `backfeed/ledger.rs` submodule
(mirroring the `dep_age/{gate,preflight}` precedent). The
markdown-header logic also overlaps `feedback.rs`'s section
detection; the RT-4 fix already extracted a shared
`helpers::is_fence`, but a fuller shared markdown-header
helper could serve both. Deferred: low priority per the
review; the production line count is borderline, not over.

### aq-2026-07-16-shared-toml-scan-helper

**Category:** Duplication (shared helper)

`coverage::coverage_section_body` re-implements the same
`[section]` header detection as `deploy_guard::section_name`
(`trimmed.starts_with('[') && trimmed.ends_with(']')` +
`trim_matches`), and `coverage::scan_quoted` overlaps
conceptually with `deploy_guard::unquote_value` /
`dep_age::gate`'s ad-hoc quote handling. Three modules now
hand-scan TOML with slightly different, drift-prone shapes.
Consider a shared dependency-free `toml_scan` module (section
detection, comment stripping, a quote-aware string/array
reader) that `coverage`, `deploy_guard`, and `gate` build on.
Deferred: unifying it touches `deploy_guard.rs` and `gate.rs`
outside the current diff, and the shapes differ enough
(single value vs array vs lockfile records) that a good shared
API needs its own design pass. Pairs with
`aq-2026-07-16-shared-git-capture-helper` -- the same
"consolidate the hand-scan primitives" theme. If a fourth
hand-parser appears, do the extraction.

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
