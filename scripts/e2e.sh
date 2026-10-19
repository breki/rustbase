#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
# shellcheck source=scripts/lib/port-utils.sh
. "$SCRIPT_DIR/lib/port-utils.sh"

# Run from the project root so `npx playwright test`
# resolves playwright.config.ts regardless of the caller's
# working directory. Without this, a caller in a
# subdirectory gets a silent "No tests found" zero-test
# pass instead of an error.
cd "$PROJECT_ROOT"

PORTS_FILE="$PROJECT_ROOT/.ports"

# Resolve an e2e port: explicit env override wins, else the
# `.ports` file, else the default. Validated as a positive
# integer here (fail fast) so the shell layer that *frees*
# the port and the JS/TS configs that *bind* it can never
# disagree -- an invalid value must not silently fall back
# to a different port in one layer but not another.
resolve_port() {
    local env_val="$1" key="$2" def="$3" val
    if [[ -n "$env_val" ]]; then
        val="$env_val"
    else
        val="$(read_port "$PORTS_FILE" "$key" "$def")"
    fi
    if [[ ! "$val" =~ ^[1-9][0-9]*$ ]]; then
        echo "e2e: invalid port '$val' for $key" >&2
        return 1
    fi
    printf '%s' "$val"
}

# The e2e harness runs on ports disjoint from the dev
# server (and from other rustbase projects, if each sets a
# distinct block in its own .ports). playwright.config.ts
# reads the same env / .ports / defaults and pushes these
# to both webServers, so a bare `npx playwright test` (no
# e2e.sh) resolves identically.
# `|| exit 1` makes the fail-fast visible at the call site
# (resolve_port `return`s non-zero on an invalid value)
# rather than relying on set -e to propagate a subshell exit.
E2E_BACKEND_PORT="$(resolve_port "${E2E_BACKEND_PORT:-}" e2e_backend_port 3001)" || exit 1
E2E_FRONTEND_PORT="$(resolve_port "${E2E_FRONTEND_PORT:-}" e2e_frontend_port 5174)" || exit 1
export E2E_BACKEND_PORT E2E_FRONTEND_PORT

# Quiet the backend by default so per-request tower_http
# DEBUG pairs don't bury the suite output. Keep the guard so
# a caller debugging a failure can still raise verbosity.
export RUST_LOG="${RUST_LOG:-rustbase_web=info,tower_http=warn}"

# Free ONLY our own e2e ports (by listener). The dev servers
# are deliberately left running -- isolation means an e2e
# run neither stops nor restarts them.
free_port "$E2E_BACKEND_PORT"
free_port "$E2E_FRONTEND_PORT"

npx playwright test "$@"
