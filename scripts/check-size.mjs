// Purpose: enforce the size limits (no source file over 250 code lines) so files stay small and focused.
// Counts non-blank lines that are not pure `//` comments in src/ and src-tauri/src/. Usage: node scripts/check-size.mjs
import { readdirSync, readFileSync, statSync } from "node:fs";
import { extname, join } from "node:path";

const MAX = 250;
const roots = ["src", "src-tauri/src"];
const exts = new Set([".rs", ".ts", ".tsx"]);

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
