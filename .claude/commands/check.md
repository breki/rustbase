---
description: Type-check all targets without running tests
allowed-tools: Bash(cargo xtask:*), Bash(cargo -q xtask:*)
---

Type-check the whole workspace, compiling every target
(library, binary, **and** tests/benches/examples) but
running no tests. Compiling the test targets is what
catches a broken call site -- an altered signature, a
renamed field -- that a lib-and-binary-only check would
miss and only surface later under `cargo xtask test`.

## Usage

`/check` -- check if code compiles

## Output

**Success:** `Check OK`
**Failure:** shows compilation errors (first 10)

## When to use

After making code changes, before running tests --
catches syntax/type errors across every target without
paying for the lint passes (`clippy`) or the test run
(`test`).

## Implementation

```
cargo -q xtask check
```
