import js from "@eslint/js";
import svelte from "eslint-plugin-svelte";
import globals from "globals";
import tsParser from "@typescript-eslint/parser";
import svelteParser from "svelte-eslint-parser";

export default [
  js.configs.recommended,
  ...svelte.configs.recommended,
  {
    languageOptions: {
      globals: { ...globals.browser },
    },
  },
  // Parse plain `.ts` files with the TypeScript parser so
  // bare TS modules lint cleanly under flat config. Exclude
  // Svelte 5 `*.svelte.ts` module files: those are already
  // handled by svelte.configs.recommended (svelte parser +
  // the svelte/svelte processor for rune globals), and this
  // later block's last-match-wins merge would otherwise
  // override the parser back to plain TS and drop the
  // processor.
  {
    files: ["**/*.ts"],
    ignores: ["**/*.svelte.ts"],
    languageOptions: { parser: tsParser },
  },
  // Parse `<script lang="ts">` blocks inside Svelte
  // components: svelte-eslint-parser handles the component,
  // delegating the script body to the TS parser. Without
  // this, TS-only syntax (`interface`, `import type`, type
  // aliases) fails with "Unexpected token" -- and the
  // starter App.svelte already declares `interface`s.
  {
    files: ["**/*.svelte"],
    languageOptions: {
      parser: svelteParser,
      parserOptions: { parser: tsParser },
    },
  },
  { ignores: ["dist/"] },
];
