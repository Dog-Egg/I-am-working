import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { externalizeDepsPlugin, defineConfig } from "electron-vite";
import { resolve } from "path";

export default defineConfig({
  main: {
    plugins: [externalizeDepsPlugin()],
    build: {
      rollupOptions: {
        input: {
          index: resolve(__dirname, "src/main/main.ts"),
        },
      },
    },
  },
  preload: {
    plugins: [externalizeDepsPlugin()],
    build: {
      rollupOptions: {
        input: {
          index: resolve(__dirname, "src/preload/prompt-preload.ts"),
        },
      },
    },
  },
  renderer: {
    root: "src/renderer",
    base: "./",
    build: {
      outDir: "../../out/renderer",
      emptyOutDir: true,
      rollupOptions: {
        input: {
          prompt: resolve(__dirname, "src/renderer/prompt.html"),
        },
      },
    },
    plugins: [svelte(), tailwindcss()],
  },
});
