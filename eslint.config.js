// Lint rules: type-aware TS + Solid reactivity, plus the project size limits (50-line functions, 250-line files).
import js from "@eslint/js";
import solid from "eslint-plugin-solid/configs/typescript";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "node_modules", "src-tauri", "vendor"] },
  js.configs.recommended,
  tseslint.configs.recommended,
  { files: ["src/**/*.{ts,tsx}"], ...solid, languageOptions: { ...solid.languageOptions, globals: globals.browser } },
  {
    files: ["src/**/*.{ts,tsx}"],
    rules: {
      "max-lines": ["error", { max: 250, skipBlankLines: true, skipComments: true }],
      "max-lines-per-function": ["error", { max: 50, skipBlankLines: true, skipComments: true }],
      "max-depth": ["error", 3],
      complexity: ["error", 10],
    },
  },
);
