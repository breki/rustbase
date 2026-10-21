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

`/validate --skip <gate>` -- leave a gate out, by its subcommand
(`coverage`, `dupes`, `test`, `frontend-test`, ...) or its name,
quoted when it has a space (`--skip "Frontend test"`); repeatable. A
name matching no gate is refused with the list of subcommands, and a
set leaving out every gate is refused. A partial run names the
skipped gates before the first step and after the closing `Validate
OK`.

Which Rust tests a skip set runs (the library, `rustbase-xtask`, runs
xtask's tests in the Test gate and every other crate's in Coverage):

- `--skip coverage` -- all of them once, in a `Test (workspace)` step
  that takes Coverage's slot and replaces the Test gate
- `--skip coverage --skip test` -- none
- `--skip test` -- every crate's but xtask's, in Coverage

The frontend tests are their own gate, `frontend-test`. Run alone, it
takes path filters: `cargo xtask frontend-test <filters>` runs only the
test files whose path contains one, and fails when no file matches;
vitest options go after `--` (`cargo xtask frontend-test -- -t <name>`).
A `-t` name that matches no test still passes: vitest counts the file
as skipped.

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
itself (Audit when the network is down, for example) appear on the
terminal only. A second run while one is still
going is refused; the `target/validate.log.lock` file left behind
is harmless. A run that cannot open the log for another reason
warns and is not logged, so the file then holds an earlier run's
output.

A frontend gate also keeps its tool's whole output, both streams, in
`target/frontend-<gate>.log` (`fmt`, `check`, `dupes`, `test`),
rewritten each run. Its first two lines are the run's npm arguments,
as a Rust list rather than a command to paste, so a filtered run shows
its filters, then the exit status. A failure's report shows only the
last few lines of the tool's output, so failing test names and counts
printed earlier are only in that log. The report's first line names the
file; under `validate` it follows the `FAILED` step line and the
"iterate with" line.

## Implementation

```
cargo -q xtask validate $ARGUMENTS
```
