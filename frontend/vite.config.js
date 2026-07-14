import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { readFileSync, existsSync } from "fs";
import { resolve } from "path";

const rootDir = resolve(import.meta.dirname, "..");

// Read app version from Cargo.toml (single source of truth)
function getAppVersion() {
  const cargoToml = resolve(rootDir, "crates", "rustbase", "Cargo.toml");
  const content = readFileSync(cargoToml, "utf-8");
  const match = content.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error("Could not find version in Cargo.toml");
  }
  return match[1];
}

// Parse a positive-integer port from an env var; undefined
// for unset / non-numeric / non-positive so a bad value
// falls back rather than binding port 0 (OS-assigned).
// (Keep in sync with the twin in playwright.config.ts.)
function envPort(name) {
  const raw = process.env[name];
  if (!raw) return undefined;
  const n = Number(raw);
  return Number.isInteger(n) && n > 0 ? n : undefined;
}

// Read <key>'s integer value from ../.ports, or <fallback>
// if the file/key is absent. (Twin in playwright.config.ts.)
function portFromFile(key, fallback) {
  const portsFile = resolve(rootDir, ".ports");
  if (!existsSync(portsFile)) return fallback;
  for (const raw of readFileSync(portsFile, "utf-8").split("\n")) {
    const line = raw.split("#")[0].replace(/\s/g, "");
    if (!line.startsWith(`${key}=`)) continue;
    const val = line.slice(key.length + 1);
    // Positive integers only -- matches e2e.sh's resolve_port
    // (rejects `0` / leading zeros) so both paths agree.
    if (/^[1-9]\d*$/.test(val)) return parseInt(val, 10);
  }
  return fallback;
}

// During an isolated e2e run the E2E_* vars are set (by
// scripts/e2e.sh, or pushed by playwright.config.ts's
// frontend webServer), so Vite binds the dedicated frontend
// port and proxies to the dedicated backend. Unset in normal
// dev, so dev uses the .ports `frontend_port` / `backend_port`
// (defaults 5173 / 3000).
const e2eFrontendPort = envPort("E2E_FRONTEND_PORT");
const frontendPort = e2eFrontendPort ?? portFromFile("frontend_port", 5173);
const backendPort =
  envPort("E2E_BACKEND_PORT") ?? portFromFile("backend_port", 3000);

export default defineConfig({
  plugins: [svelte()],
  define: {
    __APP_VERSION__: JSON.stringify(getAppVersion()),
  },
  server: {
    port: frontendPort,
    // On an isolated e2e run, fail rather than silently
    // binding the next free port: Playwright health-checks
    // the expected port and a silent shift would hang the
    // full webServer timeout. Left off in dev so a busy
    // 5173 still falls through to the next port as usual.
    strictPort: e2eFrontendPort !== undefined,
    proxy: {
      "/api": {
        target: `http://127.0.0.1:${backendPort}`,
        changeOrigin: true,
      },
      "/health": {
        target: `http://127.0.0.1:${backendPort}`,
        changeOrigin: true,
      },
    },
  },
});
