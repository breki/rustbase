import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Backend port for API proxy in dev mode.
// Change this to match your --port flag.
const backendPort = 3000;

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: `http://127.0.0.1:${backendPort}`,
        changeOrigin: true,
      },
    },
  },
});
