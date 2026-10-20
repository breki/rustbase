---
description: Run all quality checks with stepwise progress
allowed-tools: Bash(cargo xtask:*), Bash(cargo -q xtask:*)
---

Run the full validation pipeline with concise output.

## Usage

`/validate` -- run every gate listed under **Definition of Done**
in `CLAUDE.md`, in the order `step::standard`
(`crates/rustbase-xtask/src/step.rs`) gives, with the frontend
gates `xtask/src/validate.rs` inserts after Deny

## Output

One line per gate (abridged):

```
[1/15] Dep-age....... OK (no dependency changes, 0.0s)
[2/15] Fmt........... OK (0.2s)
[3/15] Canon......... OK (28 files, 5 checks, 0.0s)
[4/15] Records....... OK (5 files, 109 entries, 0.0s)
...
[14/15] Coverage...... OK (100.0% >= 90%, 47.5s)
[15/15] Audit......... OK
Validate OK
```

## Implementation

```
cargo -q xtask validate
```
