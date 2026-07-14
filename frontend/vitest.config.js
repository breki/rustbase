import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  // Mirror vite.config.js's build-time constant so components
  // that reference __APP_VERSION__ don't throw a ReferenceError
  // under vitest (which does not load the app vite config).
  define: {
    __APP_VERSION__: JSON.stringify("test"),
  },
  resolve: {
    conditions: ["browser"],
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{js,ts}"],
    passWithNoTests: true,
  },
});
