import tailwindcss from "@tailwindcss/vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import { resolve } from "path";

export default defineConfig({
  root: "src/renderer",
  base: "./",
  plugins: [svelte(), tailwindcss()],
  build: {
    outDir: "../../dist/renderer",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        prompt: resolve(__dirname, "src/renderer/prompt.html")
      }
    }
  }
});
