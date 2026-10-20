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

One line per gate (rows 6 to 14 left out):

```
[1/16] Dep-age....... OK (no dependency changes, 0.0s)
[2/16] Fmt........... OK (0.2s)
[3/16] Canon......... OK (28 files, 5 checks, 0.0s)
[4/16] Records....... OK (5 files, 109 entries, 0.0s)
[5/16] Scripts....... OK (2 scripts executable, 0.0s)
...
[15/16] Coverage...... OK (96.5% >= 90%, 10.2s)
[16/16] Audit......... OK (cargo: 0 vuln, 0 warn, npm: 0 vuln, 1.8s)
Validate OK (28.8s)
```

The same lines, and a failure's report, are also in
`target/validate.log`, rewritten each run. Warnings a gate prints
itself (Dep-age or Audit when the network is down, for example)
appear on the terminal only. A second run while one is still
going is refused; the `target/validate.log.lock` file left behind
is harmless. A run that cannot open the log for another reason
warns and is not logged, so the file then holds an earlier run's
output.

## Implementation

```
cargo -q xtask validate
```
