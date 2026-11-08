/**
 * File: scripts/check/check-size.mjs
 * Purpose: enforce the size limit (no source file over 250 code lines) so files stay small and focused. Counts non-blank lines that are not pure `//` comments in src/ and src-tauri/src/.
 * Contents: walk (file lister), the line count per file, the failure report.
 * Usage: node scripts/check/check-size.mjs (works from any directory)
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";

const MAX = 250;
const repo = fileURLToPath(new URL("../../", import.meta.url));
const roots = ["src", "src-tauri/src"].map((r) => join(repo, r));
const exts = new Set([".rs", ".ts", ".tsx"]);

/**
 * Lists the .rs, .ts and .tsx files under a folder, recursively.
 * @param {string} dir folder to walk
 * @returns {Generator<string>} file paths
 */
function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* walk(path);
    else if (exts.has(extname(path))) yield path;
  }
}

const over = [];
for (const root of roots) {
  for (const file of walk(root)) {
    const lines = readFileSync(file, "utf8").split("\n").filter((l) => l.trim() && !l.trim().startsWith("//")).length;
    if (lines > MAX) over.push(`${file}: ${lines} lines`);
  }
}
if (over.length) {
  console.error(`Files over ${MAX} code lines:\n${over.join("\n")}`);
  process.exit(1);
}
