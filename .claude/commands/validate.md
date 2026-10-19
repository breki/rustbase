---
description: Run all quality checks with stepwise progress
allowed-tools: Bash(cargo xtask:*), Bash(cargo -q xtask:*)
---

Run the full validation pipeline with concise output.

## Usage

`/validate` -- run every gate listed under **Definition of Done**
in `CLAUDE.md`, in the order `xtask/src/validate.rs` defines

## Output

One line per gate (abridged):

```
[1/14] Dep-age....... OK (no dependency changes, 0.0s)
[2/14] Fmt........... OK (0.2s)
[3/14] Canon......... OK (28 files, 5 checks, 0.0s)
...
[13/14] Coverage...... OK (100.0% >= 90%, 47.5s)
[14/14] Audit......... OK
Validate OK
```

## Implementation

```
cargo -q xtask validate
```
