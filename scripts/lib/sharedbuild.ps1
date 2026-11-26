# File: scripts/lib/sharedbuild.ps1
# Purpose: the cargo build folder that every winsync Windows copy shares (CUE_SHARED_BUILD=1, set by scripts/wsl/winsync), so parallel copies reuse compiled dependencies while each keeps its own exes. Split out of scripts/cue.ps1 so scripts/test/sharedbuild.check.ps1 can run it for real.
# Contents: Use-SharedBuild (point cargo at the shared folder, hold its lock, clean Cue's own crate on a copy switch).

# When CUE_SHARED_BUILD is 1, points cargo's build folder (intermediate artifacts) at %LOCALAPPDATA%\cue-build\<name>, shared by every Windows copy, while each copy keeps its exes in its own target folder. Holds that root's lock in $script:buildLock until the calling process exits, so copies never build at once. Cargo takes Cue's own crate as fresh when another copy built it last (its hash ignores the copy's path and its source times can be older), so when the profile's owner marker names another copy (the current location), Cue's crate is cleaned for that profile first. Inputs: the folder name, the cargo profile the mode builds, the manifest path. Returns the folder, or nothing when not shared.
function Use-SharedBuild($name, $buildProfile, $manifest) {
  if ($env:CUE_SHARED_BUILD -ne "1") { return }
  $root = Join-Path $env:LOCALAPPDATA "cue-build"
  $dir = Join-Path $root $name
  New-Item -ItemType Directory -Force $dir | Out-Null
  while (-not $script:buildLock) {
    try { $script:buildLock = [IO.File]::Open((Join-Path $root "lock"), "OpenOrCreate", "ReadWrite", "None") }
    catch { if ($_.Exception.InnerException.HResult -ne -2147024864) { throw }; Write-Host "waiting: another copy is building in $root"; Start-Sleep 5 }
  }
  $env:CARGO_BUILD_BUILD_DIR = $dir
  $owner = Join-Path $dir "copy-$buildProfile.txt"
  if ((Get-Content -LiteralPath $owner -ErrorAction SilentlyContinue) -ne $PWD.Path) {
    Write-Host "> cargo clean -p cue --profile $buildProfile (another copy built here last)" -ForegroundColor Cyan
    cargo clean --manifest-path $manifest -p cue --profile $buildProfile
    if ($LASTEXITCODE -ne 0) { throw "cargo clean -p cue --profile $buildProfile failed (exit code $LASTEXITCODE)" }
    Set-Content -LiteralPath $owner $PWD.Path
  }
  $dir
}
