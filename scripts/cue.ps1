# Purpose: one entry point to set up, run, check and build Cue on Windows, so a new contributor does not have to remember the commands.
# Usage: pwsh scripts/cue.ps1 <setup|dev|run|check|test|release|build>   (Windows PowerShell 5.1 works too)
#   setup    check the tools Cue needs and install the frontend packages
#   dev      run the app with hot reload
#   release  build the optimized exe only: src-tauri\target\release\cue.exe
#   build    build the exe and the installer: src-tauri\target\release\bundle\nsis
#   run      start the exe that `release` built
#   check    types, ESLint, size limits, rustfmt and clippy (what CI runs)
#   test     cargo test
param([Parameter(Position = 0)][ValidateSet("setup", "dev", "run", "check", "test", "release", "build")][string]$Mode = "setup")
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
$manifest = "src-tauri/Cargo.toml"

function Step($what, [scriptblock]$cmd) {
  Write-Host "> $what" -ForegroundColor Cyan
  & $cmd
  if ($LASTEXITCODE -ne 0) { throw "$what failed (exit code $LASTEXITCODE)" }
}

function Missing($tool, $how) { Write-Host "  missing: $tool. $how" -ForegroundColor Red; $script:ok = $false }

function Test-Tools {
  $script:ok = $true
  if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Missing "Rust" "Install it from https://rustup.rs (choose the MSVC toolchain)." }
  $node = Get-Command node -ErrorAction SilentlyContinue
  if (-not $node) { Missing "Node.js" "Install version 24 or newer from https://nodejs.org." }
  elseif ([int]((node -v) -replace "^v(\d+).*", '$1') -lt 24) { Missing "Node.js 24 or newer" "Found $(node -v). Update it from https://nodejs.org." }
  if (-not (Get-Command pnpm -ErrorAction SilentlyContinue) -and $node) {
    corepack enable 2>$null # pnpm ships with Node through corepack
    if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) { Missing "pnpm" 'Run "corepack enable" in an administrator shell, or "npm install -g pnpm".' }
  }
  $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  $tools = if (Test-Path $vswhere) { & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath }
  if (-not $tools) { Missing "Visual Studio Build Tools (C++)" "Install https://aka.ms/vs/17/release/vs_BuildTools.exe and tick 'Desktop development with C++'." }
  $webview = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
  if (-not (Test-Path $webview) -and -not (Test-Path ($webview -replace "^HKLM:\\SOFTWARE\\WOW6432Node", "HKCU:\SOFTWARE"))) {
    Missing "WebView2 runtime" "It comes with current Windows 10 and 11. Otherwise install it from https://developer.microsoft.com/microsoft-edge/webview2."
  }
  if (-not $script:ok) { throw "Install what is missing above, then run this again." }
  Write-Host "Tools found." -ForegroundColor Green
}

function Install-Packages {
  if (-not (Test-Path node_modules) -or $Mode -eq "setup") { Step "pnpm install" { pnpm install --frozen-lockfile } }
}

Test-Tools
Install-Packages
switch ($Mode) {
  "setup" { Write-Host "Ready. Next: scripts/cue.ps1 dev (run with hot reload) or scripts/cue.ps1 release (build the exe)." -ForegroundColor Green }
  "dev" { Step "pnpm tauri dev" { pnpm tauri dev } }
  "release" { Step "pnpm tauri build --no-bundle" { pnpm tauri build --no-bundle } }
  "build" { Step "pnpm tauri build" { pnpm tauri build } }
  "run" {
    $exe = "src-tauri\target\release\cue.exe"
    if (-not (Test-Path $exe)) { throw "Build it first: scripts/cue.ps1 release" }
    Start-Process $exe
  }
  "check" {
    Step "pnpm check" { pnpm check }
    Step "cargo fmt --check" { cargo fmt --manifest-path $manifest --check }
    Step "cargo clippy" { cargo clippy --manifest-path $manifest --all-targets -- -D warnings }
  }
  "test" { Step "cargo test" { cargo test --manifest-path $manifest } }
}
