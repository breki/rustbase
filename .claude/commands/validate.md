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
[1/16] Dep-age....... OK (no dependency changes, 0.0s)
[2/16] Fmt........... OK (0.2s)
[3/16] Canon......... OK (28 files, 5 checks, 0.0s)
[4/16] Records....... OK (5 files, 109 entries, 0.0s)
[5/16] Scripts....... OK (2 scripts executable, 0.0s)
...
[15/16] Coverage...... OK (100.0% >= 90%, 47.5s)
[16/16] Audit......... OK
Validate OK
```

## Implementation

```
cargo -q xtask validate
```
