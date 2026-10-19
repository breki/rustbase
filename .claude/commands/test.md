---
description: Run tests with minimal agent-friendly output
allowed-tools: Bash(cargo xtask:*), Bash(cargo -q xtask:*)
---

Run tests with agent-friendly output.

## Usage

- `/test` -- run all tests
- `/test search` -- run tests matching "search"

## Output

**Success:** `Test OK`
**Failure:** shows only failing tests with assertion
details

## Implementation

```
cargo -q xtask test $ARGUMENTS
```
