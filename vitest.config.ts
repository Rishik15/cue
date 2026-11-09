/**
 * File: vitest.config.ts
 * Purpose: how the frontend tests run: the same Solid transform as the app without hot reload (its refresh module breaks on Windows paths), a browser-like environment for the Tauri IPC mock, and only `*.test.ts` files under src.
 * Contents: the Vitest configuration.
 */
import { defineConfig } from "vitest/config";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid({ hot: false })],
  test: { environment: "jsdom", include: ["src/**/*.test.ts"], clearMocks: true },
});
