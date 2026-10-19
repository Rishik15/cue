// Purpose: write latest.json, the file Cue's update check reads from the newest release.
// Usage: node scripts/update-manifest.mjs <version> <installer.exe> (the installer's .sig sits beside it, made by `tauri signer sign`).
import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";

const [version, installer] = process.argv.slice(2);
if (!version || !installer) {
  console.error("usage: node scripts/update-manifest.mjs <version> <installer.exe>");
  process.exit(1);
}
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`version must look like 1.2.3, got "${version}"`);
  process.exit(1);
}
if (basename(installer) !== `Cue_${version}_x64-setup.exe`) {
  console.error(`installer must be named Cue_${version}_x64-setup.exe, got "${basename(installer)}"`);
  process.exit(1);
}
const manifest = {
  version,
  url: `https://github.com/Rishik15/cue/releases/download/v${version}/${basename(installer)}`,
  signature: readFileSync(`${installer}.sig`, "utf8").trim(),
};
writeFileSync("latest.json", `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`latest.json written for ${version}`);
