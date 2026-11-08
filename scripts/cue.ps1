# File: scripts/cue.ps1
# Purpose: the one place that defines how Cue is set up, run, checked, tested, built and measured on Windows. Contributors, CI and the WSL helper (scripts/wsl/winsync) all call it, so a command exists once.
# Contents: the usage text, the tool check, the Visual Studio environment, package install, the harness runner, one function per mode group and the mode switch.
# Usage: pwsh scripts/cue.ps1 <mode> [-Quick] [-Deps] [harness arguments]   (Windows PowerShell 5.1 works too; `pwsh scripts/cue.ps1 help` lists the modes)
param([string]$Mode = "help", [switch]$Quick, [switch]$Deps)
$ErrorActionPreference = "Stop"
$Rest = [string[]]$args
. (Join-Path $PSScriptRoot "lib\splat.ps1")
Set-Location (Join-Path $PSScriptRoot "..")
$manifest = "src-tauri/Cargo.toml"
$release = "src-tauri\target\release\cue.exe"
$debug = "src-tauri\target\debug\cue.exe"
$scratch = Join-Path $env:TEMP "cue"
New-Item -ItemType Directory -Force $scratch | Out-Null

$usage = @"
Cue build tool. Usage: scripts/cue.ps1 <mode>
  setup        check the tools Cue needs and install the frontend packages
  dev          run the app with hot reload (live: the same, hidden, log in $scratch\dev.log; stop ends it)
  check        types, ESLint, size and theme gates, rustfmt, clippy (-Quick: types and cargo check only; -Deps: also unused crates and advisories)
  test         cargo test         audit   pnpm audit --prod
  release      optimized exe only         build   exe and installer         run   start the release exe
  bench smoke threads tray-release selection soak leak   end-to-end checks on the release exe (build it first)
  smoke-debug tray insert    the same kind of checks on a debug build (built first)
  installer    install, run and uninstall the built installer
  lab <name>   maintainer experiments: models textmodels texttasks textsecurity
"@

# Prints a step name, runs a script block and throws when its exit code is not 0. Inputs: description, script block.
function Step($what, [scriptblock]$cmd) {
  Write-Host "> $what" -ForegroundColor Cyan
  & $cmd
  if ($LASTEXITCODE -ne 0) { throw "$what failed (exit code $LASTEXITCODE)" }
}

# Prints one missing tool with how to get it and marks the tool check as failed. Inputs: tool name, install hint.
function Missing($tool, $how) { Write-Host "  missing: $tool. $how" -ForegroundColor Red; $script:ok = $false }

# Checks that Rust, Node 24+, pnpm, the Visual Studio C++ tools and WebView2 are installed; throws when something is missing.
function Test-Tools {
  $script:ok = $true
  if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Missing "Rust" "Install it from https://rustup.rs (choose the MSVC toolchain)." }
  $node = Get-Command node -ErrorAction SilentlyContinue
  if (-not $node) { Missing "Node.js" "Install version 24 or newer from https://nodejs.org." }
  elseif ([int]((node -v) -replace "^v(\d+).*", '$1') -lt 24) { Missing "Node.js 24 or newer" "Found $(node -v). Update it from https://nodejs.org." }
  if (-not (Get-Command pnpm -ErrorAction SilentlyContinue) -and $node) {
    corepack enable 2>$null
    if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) { Missing "pnpm" 'Run "corepack enable" in an administrator shell, or "npm install -g pnpm".' }
  }
  if (-not (Get-VsRoot)) { Missing "Visual Studio Build Tools (C++)" "Install https://aka.ms/vs/17/release/vs_BuildTools.exe and tick 'Desktop development with C++'." }
  $webview = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
  if (-not (Test-Path $webview) -and -not (Test-Path ($webview -replace "^HKLM:\\SOFTWARE\\WOW6432Node", "HKCU:\SOFTWARE"))) {
    Missing "WebView2 runtime" "It comes with current Windows 10 and 11. Otherwise install it from https://developer.microsoft.com/microsoft-edge/webview2."
  }
  if (-not $script:ok) { throw "Install what is missing above, then run this again." }
  Write-Host "Tools found." -ForegroundColor Green
}

# Returns the Visual Studio install folder that has the C++ tools, or nothing.
function Get-VsRoot {
  $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path $vswhere) { & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1 }
}

# Loads the Visual Studio build environment variables into this session unless they are already set.
function Import-VsEnv {
  if ($env:VSCMD_VER) { return }
  $root = Get-VsRoot
  $vcvars = if ($root) { Join-Path $root "VC\Auxiliary\Build\vcvars64.bat" }
  if (-not $vcvars -or -not (Test-Path $vcvars)) { return }
  cmd /c "`"$vcvars`" >nul && set" | ForEach-Object { if ($_ -match "^([^=]+)=(.*)$") { Set-Item "env:$($Matches[1])" $Matches[2] } }
}

# Runs pnpm install when node_modules is older than pnpm-lock.yaml or in setup mode.
function Install-Packages {
  $marker = "node_modules\.modules.yaml"
  $stale = -not (Test-Path $marker) -or ((Get-Item pnpm-lock.yaml).LastWriteTime -gt (Get-Item $marker).LastWriteTime)
  if ($stale -or $Mode -eq "setup") { Step "pnpm install" { pnpm install --frozen-lockfile } }
}

# Stops whatever listens on the Vite port 1420, with its child processes.
function Stop-Vite {
  Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue | ForEach-Object { cmd /c "taskkill /F /T /PID $($_.OwningProcess) >nul 2>&1" }
}

# Runs a verify script with the extra command-line words as named parameters and exits with its code on failure. Inputs: script path under scripts/, extra parameters.
function Invoke-Harness($path, [hashtable]$extra = @{}) {
  $splat = ConvertTo-Splat $Rest
  foreach ($k in $extra.Keys) { $splat[$k] = $extra[$k] }
  & (Join-Path $PSScriptRoot $path) @splat
  if ($LASTEXITCODE) { exit $LASTEXITCODE }
}

# Throws unless the release exe has been built.
function Need-Release {
  if (-not (Test-Path $release)) { throw "Build it first: scripts/cue.ps1 release" }
}

# Runs the checks: types and cargo check with -Quick, otherwise pnpm check, rustfmt and clippy; adds Invoke-Deps with -Deps.
function Invoke-Check {
  if ($Quick) {
    Step "pnpm typecheck" { pnpm typecheck }
    Step "cargo check" { cargo check --manifest-path $manifest }
  } else {
    Step "pnpm check" { pnpm check }
    Step "cargo fmt --check" { cargo fmt --manifest-path $manifest --check }
    Step "cargo clippy" { cargo clippy --manifest-path $manifest --all-targets -- -D warnings }
  }
  if ($Deps) { Invoke-Deps }
}

# Runs cargo machete and cargo deny; throws when a tool is missing.
function Invoke-Deps {
  foreach ($tool in "cargo-machete", "cargo-deny") {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "$tool is missing. Install it with: cargo install $tool" }
  }
  Step "cargo machete" { cargo machete src-tauri }
  Step "cargo deny" { cargo deny --manifest-path $manifest --config src-tauri/deny.toml check advisories bans sources }
}

# Runs cargo test in its own target folder with a static CRT. shortcut: the separate target directory and static CRT came from the old winsync and the cause of the TaskDialogIndirect start-up failure they work around is unverified, check on Windows before simplifying.
function Invoke-Test {
  $env:CARGO_TARGET_DIR = "src-tauri\target\test"
  $env:RUSTFLAGS = "-C target-feature=+crt-static"
  Step "cargo test" { cargo test --manifest-path $manifest }
}

# Starts `dev` in a hidden PowerShell with its output in the scratch log.
function Start-Live {
  $log = Join-Path $scratch "dev.log"
  $cmd = "& '$PSCommandPath' dev *> '$log'"
  Start-Process powershell -WindowStyle Hidden -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", $cmd
  Write-Host "live: tauri dev started, log $log; scripts/cue.ps1 stop ends it"
}

# Runs the chosen mode (dev, release, build, check, test, audit and the end-to-end checks).
function Invoke-Mode {
  switch ($Mode) {
    "dev" { Stop-Vite; try { Step "pnpm tauri dev" { pnpm tauri dev } } finally { Stop-Vite } }
    "release" { Step "pnpm tauri build --no-bundle" { pnpm tauri build --no-bundle } }
    "build" { Step "pnpm tauri build" { pnpm tauri build } }
    "check" { Invoke-Check }
    "test" { Invoke-Test }
    "audit" { Step "pnpm audit" { pnpm audit --prod } }
    "bench" { Need-Release; Invoke-Harness "verify\bench.ps1" }
    "smoke" { Need-Release; Invoke-Harness "verify\smoke.ps1" }
    "threads" { Need-Release; Invoke-Harness "verify\threads.ps1" }
    "selection" { Need-Release; Invoke-Harness "verify\selection-check.ps1" }
    "soak" { Need-Release; Invoke-Harness "verify\soak.ps1" }
    "leak" { Need-Release; Invoke-Harness "verify\leak-check.ps1" }
    "tray-release" { Need-Release; Invoke-Harness "verify\tray-smoke.ps1" @{ exe = (Resolve-Path $release).Path; shot = (Join-Path $scratch "menu.png") } }
    "installer" { Invoke-Harness "verify\installer-check.ps1" }
    "smoke-debug" { Step "cargo build" { cargo build --manifest-path $manifest }; Invoke-Harness "verify\smoke.ps1" @{ exe = (Resolve-Path $debug).Path } }
    "tray" { Step "cargo build" { cargo build --manifest-path $manifest }; Invoke-Harness "verify\tray-smoke.ps1" @{ shot = (Join-Path $scratch "menu.png") } }
    "insert" { Step "cargo build" { cargo build --manifest-path $manifest }; Invoke-Harness "verify\insert-check.ps1" }
    "lab" { & (Join-Path $PSScriptRoot "lab\lab.ps1") -Name $Rest[0] -Rest ($Rest | Select-Object -Skip 1) -Scratch $scratch; if ($LASTEXITCODE) { exit $LASTEXITCODE } }
  }
}

$known = "setup", "dev", "live", "stop", "run", "check", "test", "audit", "release", "build", "bench", "smoke", "threads", "selection", "soak", "leak", "tray-release", "installer", "smoke-debug", "tray", "insert", "lab"
if ($known -notcontains $Mode) { Write-Host $usage; exit $(if ($Mode -eq "help") { 0 } else { 2 }) }

switch ($Mode) {
  "stop" { cmd /c "taskkill /F /IM cue.exe >nul 2>&1"; Stop-Vite; exit 0 }
  "run" { Need-Release; Start-Process (Resolve-Path $release).Path; exit 0 }
  "live" { Test-Tools; Install-Packages; Start-Live; exit 0 }
}
Test-Tools
Install-Packages
if ($Mode -eq "setup") { Write-Host "Ready. Next: scripts/cue.ps1 dev (run with hot reload) or scripts/cue.ps1 release (build the exe)." -ForegroundColor Green; exit 0 }
Import-VsEnv
Invoke-Mode
