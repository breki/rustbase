#!/usr/bin/env bash
# Free the dev-server ports (backend + Vite) by stopping
# whatever process is *listening* on them.
#
# Port-scoped, never by process name. A by-name kill
# (`taskkill /IM <bin>.exe`, `pkill -x <bin>`) is
# machine-wide -- it ignores ports -- so if you also run a
# production instance of the same binary on another port
# (dogfooding real data on :3000 while a worktree develops
# elsewhere), a by-name cleanup would terminate the
# production server too. Freeing only the process that
# holds this checkout's dev ports avoids that.
#
# Safe to run when nothing is listening.
#
# NOTE: the port helpers below are intentionally inline.
# When the e2e harness gains isolated ports it should grow
# a shared `scripts/lib/port-utils.sh`; these two functions
# are the seed for that extraction.
set -euo pipefail

# Dev-server ports. Backend (:3000) and Vite (:5173)
# defaults; the e2e harness frees its own ports separately.
BACKEND_PORT=3000
FRONTEND_PORT=5173

# Stop the process listening on $1 (Windows / Git Bash).
free_port_windows() {
    local port="$1"
    powershell -NoProfile -Command \
        "Get-NetTCPConnection -LocalPort $port -State Listen \
         -ErrorAction SilentlyContinue | \
         Select-Object -ExpandProperty OwningProcess -Unique | \
         ForEach-Object { Stop-Process -Id \$_ -Force \
         -ErrorAction SilentlyContinue }" 2>/dev/null || true
}

# Stop the process listening on $1 (Unix). Filters to the
# LISTEN socket so an unrelated client connection to the
# same port is never killed.
free_port_unix() {
    local port="$1"
    local pids
    pids="$(lsof -ti "tcp:$port" -sTCP:LISTEN 2>/dev/null || true)"
    if [[ -n "$pids" ]]; then
        # shellcheck disable=SC2086 -- pids is a space list
        kill $pids 2>/dev/null || true
    fi
}

if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" ]]; then
    free_port_windows "$BACKEND_PORT"
    free_port_windows "$FRONTEND_PORT"
else
    free_port_unix "$BACKEND_PORT"
    free_port_unix "$FRONTEND_PORT"
fi

sleep 1
echo "Freed dev ports $BACKEND_PORT and $FRONTEND_PORT."
