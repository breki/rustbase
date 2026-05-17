#!/usr/bin/env bash
# Stop hook: runs a fast-path subset of validate when
# Rust files have been modified -- clippy + tests only.
#
# Coverage and duplication are intentionally skipped:
# coverage alone adds ~15s per invocation on a small
# codebase, and the Stop hook fires often enough during
# interactive work that the cost compounds. Full
# `cargo xtask validate` still runs from /commit and
# is available manually for explicit pre-flight checks.
#
# fmt-check is also skipped -- formatting is virtually
# always correct in interactive flows, and /commit's
# full validate catches any drift before a commit lands.
#
# Exit codes:
#   0 -- all checks passed (or nothing to check)
#   2 -- a check failed; stderr carries the failure output
#       so Claude can fix the issues before stopping

set -euo pipefail

# --- Guard against infinite loops -------------------------
# If Claude is already fixing issues from a previous hook
# run, skip re-checking to avoid fix-fail cycles.
input="$(cat)"
if echo "$input" | grep -q '"stop_hook_active"'; then
  exit 0
fi

# --- Detect modified Rust files ---------------------------
changed_rs=$(
  {
    git diff --name-only --diff-filter=ACMR HEAD -- '*.rs' 2>/dev/null
    git diff --name-only --diff-filter=ACMR -- '*.rs' 2>/dev/null
    git ls-files --others --exclude-standard -- '*.rs' 2>/dev/null
  } | sort -u
)

if [ -z "$changed_rs" ]; then
  exit 0
fi

# --- Run checks ------------------------------------------
output=$(cargo xtask clippy 2>&1 && cargo xtask test 2>&1) || {
  echo "$output" >&2
  exit 2
}
