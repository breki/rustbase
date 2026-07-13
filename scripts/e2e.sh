#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Run from the project root so `npx playwright test`
# resolves playwright.config.ts regardless of the caller's
# working directory. Without this, a caller sitting in a
# subdirectory gets a silent "No tests found" zero-test
# pass instead of an error.
cd "$PROJECT_ROOT"

# Quiet the backend by default so the per-request
# tower_http DEBUG pairs don't bury the suite's own output
# (Playwright echoes backend stdout with a [WebServer]
# prefix). Keep the ${RUST_LOG:-...} guard so a caller
# debugging a failure can still raise verbosity, e.g.
# `RUST_LOG=tower_http=debug scripts/e2e.sh`. Inherited by
# the `cargo run` backend child Playwright spawns.
export RUST_LOG="${RUST_LOG:-rustbase_web=info,tower_http=warn}"

# Kill stale servers to avoid port conflicts.
"$SCRIPT_DIR/kill-servers.sh"

npx playwright test "$@"
