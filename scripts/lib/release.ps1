# File: scripts/lib/release.ps1
# Purpose: tell whether the release exe is older than the source it is built from, so a harness never runs a stale build after a source sync (ticket 62).
# Contents: $ReleaseSources, Get-StaleSource (pure apart from reading file times).

# The folders and manifests a release build reads, relative to the repo root.
$ReleaseSources = @("src-tauri/src", "src", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock", "src-tauri/build.rs", "src-tauri/tauri.conf.json", "package.json", "pnpm-lock.yaml")

# Finds a source file written after the exe. Inputs: repo root, path of an existing exe. Return: the first newer file's full path, or "" when none is newer.
function Get-StaleSource($root, $exe) {
  $built = (Get-Item $exe).LastWriteTimeUtc
  foreach ($source in $ReleaseSources) {
    $path = Join-Path $root $source
    if (-not (Test-Path $path)) { continue }
    $newer = Get-ChildItem $path -Recurse -File | Where-Object { $_.LastWriteTimeUtc -gt $built } | Select-Object -First 1
    if ($newer) { return $newer.FullName }
  }
  ""
}
