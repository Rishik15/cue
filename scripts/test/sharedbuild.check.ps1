# File: scripts/test/sharedbuild.check.ps1
# Purpose: self-check of scripts/lib/sharedbuild.ps1 with real cargo builds of two throwaway copies of a one-file crate named cue, sharing a build folder in a throwaway LOCALAPPDATA: nothing is set without CUE_SHARED_BUILD; copies built A, B, A (B's sources deliberately older) each run their own code; Cue's crate is cleaned only when a profile's last builder was another copy, with dev and release tracked apart; a process that took the lock and then failed leaves it free.
# Contents: $envNames, Restore-Env, New-Copy, Build, the four cases, the verdict.
# Usage: & scripts/test/sharedbuild.check.ps1   (cue.ps1 test runs it; it never touches the real %LOCALAPPDATA%\cue-build)
$ErrorActionPreference = "Stop"
$envNames = "CUE_SHARED_BUILD", "LOCALAPPDATA", "CARGO_BUILD_BUILD_DIR", "CARGO_TARGET_DIR"
$saved = @{}; foreach ($n in $envNames) { $saved[$n] = [Environment]::GetEnvironmentVariable($n) }
$root = Join-Path ([IO.Path]::GetTempPath()) ("cue-sharedbuild-check-" + [guid]::NewGuid())
$exe = if ($env:OS -eq "Windows_NT") { "cue.exe" } else { "cue" }
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\sharedbuild.ps1")

# Puts every variable in $envNames back as it was before the check.
function Restore-Env { foreach ($n in $envNames) { [Environment]::SetEnvironmentVariable($n, $saved[$n]) } }

# Writes a crate named cue whose program prints the given word. Inputs: folder name under $root, word, whether its files get a 2001 time. Returns the folder.
function New-Copy($name, $word, $old) {
  $dir = Join-Path $root $name
  New-Item -ItemType Directory -Force (Join-Path $dir "src") | Out-Null
  Set-Content -LiteralPath (Join-Path $dir "Cargo.toml") "[package]`nname = `"cue`"`nversion = `"0.1.0`"`nedition = `"2021`""
  Set-Content -LiteralPath (Join-Path $dir "src\main.rs") "fn main() { println!(`"$word`"); }"
  if ($old) { foreach ($f in "Cargo.toml", "src\main.rs") { (Get-Item -LiteralPath (Join-Path $dir $f)).LastWriteTime = [datetime]"2001-01-01" } }
  $dir
}

# Runs Use-SharedBuild and cargo build in a copy, then the copy's own exe. Inputs: copy folder, cargo profile (dev or release). Returns whether Cue's crate was cleaned first and what the exe printed.
function Build($copy, $buildProfile) {
  Push-Location $copy
  try {
    $said = Use-SharedBuild "main" $buildProfile "Cargo.toml" 6>&1 | Where-Object { $_ -is [Management.Automation.InformationRecord] }
    $release = $buildProfile -eq "release"
    if ($release) { cargo build -q --release } else { cargo build -q }
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed in $copy" }
    @{ Cleaned = [bool]($said | Where-Object { "$($_.MessageData)" -like "> cargo clean*" }); Printed = "$(& (Join-Path $copy "target\$(if ($release) { 'release' } else { 'debug' })\$exe"))" }
  } finally { Pop-Location }
}

try {
  New-Item -ItemType Directory -Force (Join-Path $root "appdata") | Out-Null
  $env:LOCALAPPDATA = Join-Path $root "appdata"
  $shared = Join-Path $env:LOCALAPPDATA "cue-build"
  Remove-Item Env:CUE_SHARED_BUILD, Env:CARGO_BUILD_BUILD_DIR, Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
  $a = New-Copy "a" "copy A" $false
  $b = New-Copy "b" "copy B" $true

  Push-Location $a; try { $none = Use-SharedBuild "main" "dev" "Cargo.toml" } finally { Pop-Location }
  Check "unshared: returns nothing" ($null -eq $none)
  Check "unshared: build folder and lock are untouched" ((-not $env:CARGO_BUILD_BUILD_DIR) -and -not (Test-Path -LiteralPath $shared) -and -not $script:buildLock)

  $env:CUE_SHARED_BUILD = "1"
  $null = Build $a "dev"
  Check "shared: cargo's build folder is the shared main folder" ($env:CARGO_BUILD_BUILD_DIR -eq (Join-Path $shared "main"))
  $r = Build $b "dev"; Check "A then B (older sources): B is cleaned" $r.Cleaned; Check "A then B (older sources): B runs its own code" ($r.Printed -eq "copy B")
  $r = Build $a "dev"; Check "B then A: A is cleaned" $r.Cleaned; Check "B then A: A runs its own code" ($r.Printed -eq "copy A")
  $r = Build $a "dev"; Check "A again: no clean" (-not $r.Cleaned); Check "A again: A runs its own code" ($r.Printed -eq "copy A")
  $r = Build $b "release"; Check "B release: B runs its own code" ($r.Printed -eq "copy B")
  $r = Build $a "dev"; Check "dev after B release: no clean (profiles apart)" (-not $r.Cleaned); Check "dev after B release: A runs its own code" ($r.Printed -eq "copy A")
  $r = Build $a "release"; Check "A release after B release: A is cleaned" $r.Cleaned; Check "A release after B release: A runs its own code" ($r.Printed -eq "copy A")

  $script:buildLock.Dispose(); $script:buildLock = $null
  $lock = Join-Path $shared "lock"; $probe = Join-Path $root "probe.txt"; $child = Join-Path $root "child.ps1"
  Set-Content -LiteralPath $child 'param($lib, $copy, $lock, $probe); $ErrorActionPreference = "Stop"; . $lib; Set-Location -LiteralPath $copy; Use-SharedBuild "main" "dev" "Cargo.toml" | Out-Null
$(try { [IO.File]::Open($lock, "Open", "ReadWrite", "ReadWrite").Dispose(); "free" } catch { "held" }) | Set-Content -LiteralPath $probe
throw "deliberate failure"'
  $run = Start-Process (Get-Process -Id $PID).Path "-NoProfile -ExecutionPolicy Bypass -File `"$child`" `"$PSScriptRoot\..\lib\sharedbuild.ps1`" `"$a`" `"$lock`" `"$probe`"" -NoNewWindow -Wait -PassThru -RedirectStandardOutput (Join-Path $root "out.txt") -RedirectStandardError (Join-Path $root "err.txt")
  Check "failing process: fails at its deliberate throw" (($run.ExitCode -ne 0) -and ((Get-Content -Raw -LiteralPath (Join-Path $root "err.txt")) -like "*deliberate failure*"))
  Check "failing process: held the lock while it ran" ((Get-Content -LiteralPath $probe -ErrorAction SilentlyContinue) -eq "held")
  $free = $false; try { [IO.File]::Open($lock, "Open", "ReadWrite", "None").Dispose(); $free = $true } catch { }
  Check "failing process: the lock is free once it exits" $free
} finally {
  if ($script:buildLock) { $script:buildLock.Dispose(); $script:buildLock = $null }
  Restore-Env
  Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
Complete-Checks
