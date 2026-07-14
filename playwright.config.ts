import { defineConfig, devices } from "@playwright/test";
import { readFileSync, existsSync } from "fs";
import { resolve } from "path";

/**
 * Walk up from process.cwd() until we find Cargo.toml.
 */
function findProjectRoot() {
  let dir = process.cwd();
  for (let i = 0; i < 10; i++) {
    if (existsSync(resolve(dir, "Cargo.toml")))
      return dir;
    const parent = resolve(dir, "..");
    if (parent === dir) break;
    dir = parent;
  }
  return process.cwd();
}

const projectRoot = findProjectRoot();

/**
 * Parse a positive-integer port from an env var. Returns
 * undefined for unset / non-numeric / non-positive values
 * so a bad value (`"0"` -> OS-assigned, `"abc"` -> NaN)
 * falls back instead of producing an unreachable health
 * URL. (Keep in sync with the twin in
 * frontend/vite.config.js -- no shared module across the
 * TS/JS config boundary without a build step.)
 */
function envPort(name: string) {
  const raw = process.env[name];
  if (!raw) return undefined;
  const n = Number(raw);
  return Number.isInteger(n) && n > 0 ? n : undefined;
}

/**
 * Read <key>'s integer value from the .ports file, or
 * <fallback> if the file/key is absent. (Twin in
 * frontend/vite.config.js.)
 */
function portFromFile(key: string, fallback: number) {
  const portsFile = resolve(projectRoot, ".ports");
  if (!existsSync(portsFile)) return fallback;
  for (const raw of readFileSync(portsFile, "utf-8").split("\n")) {
    const line = raw.split("#")[0].replace(/\s/g, "");
    if (!line.startsWith(`${key}=`)) continue;
    const val = line.slice(key.length + 1);
    // Positive integers only -- matches e2e.sh's
    // resolve_port, so a `0` / leading-zero value in .ports
    // is rejected identically on both paths rather than
    // binding port 0 (OS-assigned) on the bare-npx path.
    if (/^[1-9]\d*$/.test(val)) return parseInt(val, 10);
  }
  return fallback;
}

// This is the e2e harness config, so it always resolves the
// *isolated* ports -- env override (set by scripts/e2e.sh)
// wins, else the .ports `e2e_*` keys, else the defaults. It
// never falls back to the dev ports, and it pushes the
// resolved values to both webServers (below) so a bare
// `npx playwright test` -- with no e2e.sh -- self-isolates
// identically.
const backendPort =
  envPort("E2E_BACKEND_PORT") ?? portFromFile("e2e_backend_port", 3001);
const frontendPort =
  envPort("E2E_FRONTEND_PORT") ?? portFromFile("e2e_frontend_port", 5174);

/**
 * @see https://playwright.dev/docs/test-configuration
 */
export default defineConfig({
  testDir: "./e2e/tests",

  timeout: 30 * 1000,
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: "html",

  use: {
    baseURL: `http://localhost:${frontendPort}`,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],

  webServer: [
    {
      command: `cargo run -p rustbase-web -- --port ${backendPort}`,
      cwd: projectRoot,
      url: `http://localhost:${backendPort}/health`,
      env: {
        RUST_LOG:
          process.env.RUST_LOG ?? "rustbase_web=info,tower_http=warn",
      },
      // Never reuse a server already answering on this port
      // -- a stale orphan or a foreign process would proxy
      // the suite to the wrong backend. On the isolated e2e
      // ports there is nothing legitimate to reuse anyway.
      reuseExistingServer: false,
      stdout: "pipe",
      timeout: 120 * 1000,
    },
    {
      command: "npm run dev",
      cwd: resolve(projectRoot, "frontend"),
      url: `http://localhost:${frontendPort}`,
      // Push the resolved e2e ports so Vite binds the e2e
      // frontend port and proxies to the e2e backend, even
      // on a bare `npx playwright test` where e2e.sh did not
      // set these in the environment.
      env: {
        E2E_FRONTEND_PORT: String(frontendPort),
        E2E_BACKEND_PORT: String(backendPort),
      },
      reuseExistingServer: false,
      stdout: "pipe",
      timeout: 120 * 1000,
    },
  ],
});
