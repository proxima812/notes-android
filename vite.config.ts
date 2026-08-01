import path from "node:path";
import process from "node:process";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

const host = process.env["TAURI_DEV_HOST"];

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],

  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "src"),
    },
  },

  // Prevent Vite from obscuring Rust errors.
  clearScreen: false,

  server: {
    // Tauri expects a fixed port and fails if it is not available.
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host === undefined ? undefined : { protocol: "ws", host, port: 1421 },
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },

  build: {
    // Android WebView on Pixel 8a is Chromium-based and well ahead of this,
    // but Tauri's Linux/Windows targets are the constraint for a shared build.
    target: "es2022",
    sourcemap: false,
    chunkSizeWarningLimit: 1200,
  },

  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/shared/test/setup.ts"],
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
});
