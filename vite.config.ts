import { defineConfig } from "vitest/config";

// Tauri expects a fixed dev port and no screen clearing (keeps Rust errors visible).
export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "es2022",
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: { environment: "jsdom" },
});
