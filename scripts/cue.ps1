# File: scripts/cue.ps1
# Purpose: the one place that defines how Cue is set up, run, checked, tested, built and measured. Contributors, CI and the WSL helper (scripts/wsl/winsync) all call it, so a command exists once. Frontend modes also run under PowerShell 7 on Linux; everything else needs Windows.
# Contents: the doctor (Test-Tools), the Visual Studio environment, package install, the harness runner, check, test (with Invoke-ScriptChecks, the scripts/test self-checks) and verify, the mode table (each mode's tool needs, help line and action: the one list of modes), Write-Usage and the dispatch.
# Usage: powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\cue.ps1 <mode> [-Quick] [-Deps] [harness arguments]   (`help` lists the modes)
param([string]$Mode = "help", [switch]$Quick, [switch]$Deps)
$ErrorActionPreference = "Stop"
$Rest = [string[]]$args
. (Join-Path $PSScriptRoot "lib/splat.ps1")
. (Join-Path $PSScriptRoot "lib/sharedbuild.ps1")
. (Join-Path $PSScriptRoot "lib/release.ps1")
Set-Location (Join-Path $PSScriptRoot "..")
$manifest = "src-tauri/Cargo.toml"
$release = "src-tauri/target/release/cue.exe"
$debug = "src-tauri/target/debug/cue.exe"
$scratch = Join-Path ([IO.Path]::GetTempPath()) "cue"
New-Item -ItemType Directory -Force $scratch | Out-Null

# Prints a step name, runs a script block and throws when its exit code is not 0. Inputs: description, script block.
function Step($what, [scriptblock]$cmd) {
  Write-Host "> $what" -ForegroundColor Cyan
  & $cmd
  if ($LASTEXITCODE -ne 0) { throw "$what failed (exit code $LASTEXITCODE)" }
}

# Prints one found tool with its version. Inputs: tool name, version text.
function Found($tool, $version) { Write-Host "  ok: $tool $version" -ForegroundColor DarkGray }

# Prints one missing tool with how to get it and marks the doctor as failed. Inputs: tool name, install hint.
function Missing($tool, $how) { Write-Host "  missing: $tool. $how" -ForegroundColor Red; $script:ok = $false }

# Returns the first line a command prints, or an empty string when it cannot run. Input: script block.
function Version([scriptblock]$cmd) { try { [string](& $cmd 2>$null | Select-Object -First 1) } catch { "" } }

# Checks the Rust toolchain: cargo, the MSVC host, rustfmt and clippy.
function Test-Rust {
  if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Missing "Rust" "Install it from https://rustup.rs and choose the MSVC toolchain."; return }
  Found "Rust" (Version { rustc --version })
  if ((Version { rustc -vV }) -and -not ((rustc -vV) -match "host: .*windows-msvc")) { Missing "the MSVC Rust toolchain" "Run: rustup default stable-x86_64-pc-windows-msvc" }
  if (-not (Version { rustfmt --version })) { Missing "rustfmt" "Run: rustup component add rustfmt" }
  if (-not (Version { cargo clippy --version })) { Missing "clippy" "Run: rustup component add clippy" }
}

# Checks Node 24+, pnpm, and Corepack as the way to get the pinned pnpm.
function Test-Node {
  $node = Get-Command node -ErrorAction SilentlyContinue
  if (-not $node) { Missing "Node.js" "Install version 24 or newer from https://nodejs.org."; return }
  $v = node -v
  if ([int]($v -replace "^v(\d+).*", '$1') -lt 24) { Missing "Node.js 24 or newer" "Found $v. Update it from https://nodejs.org." } else { Found "Node.js" $v }
  if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
    if (Get-Command corepack -ErrorAction SilentlyContinue) { corepack enable 2>$null }
    if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
      Missing "pnpm" $(if (Get-Command corepack -ErrorAction SilentlyContinue) { 'Run "corepack enable" in an administrator shell.' } else { 'Corepack is also missing. Run "npm install -g corepack", then "corepack enable".' })
      return
    }
  }
  Found "pnpm" (Version { pnpm -v })
}

# Checks the Visual Studio C++ tools and the WebView2 runtime.
function Test-VisualStudio {
  if (Get-VsRoot) { Found "Visual Studio C++ tools" "" } else { Missing "Visual Studio Build Tools (C++)" "Install https://aka.ms/vs/17/release/vs_BuildTools.exe and tick 'Desktop development with C++'." }
}

# Checks that the WebView2 runtime is installed.
function Test-WebView {
  $webview = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
  if ((Test-Path $webview) -or (Test-Path ($webview -replace "^HKLM:\\SOFTWARE\\WOW6432Node", "HKCU:\SOFTWARE"))) { Found "WebView2 runtime" "" }
  else { Missing "WebView2 runtime" "It comes with current Windows 10 and 11. Otherwise install it from https://developer.microsoft.com/microsoft-edge/webview2." }
}

# Checks cargo-machete and cargo-deny, which verify and check -Deps need. Input: whether a missing tool fails the doctor.
function Test-DepTools($required) {
  foreach ($t in "cargo-machete", "cargo-deny") {
    if (Get-Command $t -ErrorAction SilentlyContinue) { Found $t ""; continue }
    if ($required) { Missing $t "Run: cargo install $t --locked" }
    else { Write-Host "  note: $t is missing; verify and check -Deps need it. Run: cargo install $t --locked" -ForegroundColor Yellow }
  }
}

# Runs every check the mode needs, prints all that is missing in one pass and throws when anything is. Input: list of needs.
function Test-Tools($want) {
  $script:ok = $true
  if ($want -contains "rust") { Test-Rust }
  if ($want -contains "node") { Test-Node }
  if ($want -contains "vs") { Test-VisualStudio }
  if ($want -contains "webview") { Test-WebView }
  if ($want -contains "deptools") { Test-DepTools $true } elseif ($Mode -eq "setup") { Test-DepTools $false }
  if (-not $script:ok) { throw "Install what is missing above, then run this again." }
}

# Returns the Visual Studio install folder that has the C++ tools, or nothing.
function Get-VsRoot {
  $pf = ${env:ProgramFiles(x86)}
  if (-not $pf) { return }
  $vswhere = Join-Path $pf "Microsoft Visual Studio\Installer\vswhere.exe"
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

# Runs pnpm install when node_modules is missing or older than the lockfile or package.json, or in setup mode.
function Install-Packages {
  $marker = "node_modules/.modules.yaml"
  $stale = $true
  if (Test-Path -LiteralPath $marker) {
    $built = (Get-Item -Force -LiteralPath $marker).LastWriteTime
    $stale = ((Get-Item pnpm-lock.yaml).LastWriteTime -gt $built) -or ((Get-Item package.json).LastWriteTime -gt $built)
  }
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

# Throws unless the release exe exists and no source file or manifest is newer than it.
function Need-Release {
  if (-not (Test-Path $release)) { throw "Build it first: scripts\cue.ps1 release" }
  $stale = Get-StaleSource (Get-Location).Path $release
  if ($stale) { throw "$stale is newer than the release exe. Build it first: scripts\cue.ps1 release" }
}

# Runs the frontend checks: types, ESLint and theme contrast.
function Invoke-FrontendCheck { Step "pnpm check" { pnpm check } }

# Runs the checks: types and cargo check with -Quick, otherwise the frontend checks, rustfmt and clippy; adds Invoke-Deps with -Deps.
function Invoke-Check {
  if ($Quick) {
    Step "pnpm typecheck" { pnpm typecheck }
    Step "cargo check" { cargo check --manifest-path $manifest }
  } else {
    Invoke-FrontendCheck
    Step "cargo fmt --check" { cargo fmt --manifest-path $manifest --check }
    Step "cargo clippy" { cargo clippy --manifest-path $manifest --all-targets -- -D warnings }
  }
  if ($Deps) { Invoke-Deps }
}

# Runs cargo machete and cargo deny; throws when a tool is missing.
function Invoke-Deps {
  $script:ok = $true
  Test-DepTools $true
  if (-not $script:ok) { throw "Install what is missing above, then run this again." }
  Step "cargo machete" { cargo machete src-tauri }
  Step "cargo deny" { cargo deny --manifest-path $manifest --config src-tauri/deny.toml check advisories bans sources }
}

# Runs cargo test in its own target folder with a static CRT. shortcut: the separate target directory and static CRT came from the old winsync and the cause of the TaskDialogIndirect start-up failure they work around is unverified, check on Windows before simplifying.
function Invoke-Test {
  $shared = Use-SharedBuild "test" "dev" $manifest
  $env:CARGO_TARGET_DIR = if ($shared) { $shared } else { "src-tauri\target\test" }
  $env:RUSTFLAGS = "-C target-feature=+crt-static"
  Step "cargo test" { cargo test --manifest-path $manifest }
  Step "script self-checks" { Invoke-ScriptChecks }
}

# Runs every scripts/test/*.check.ps1 and throws "<name> failed" when one exits with a non-zero code.
function Invoke-ScriptChecks {
  foreach ($check in Get-ChildItem (Join-Path $PSScriptRoot "test") -Filter *.check.ps1) {
    $global:LASTEXITCODE = 0
    & $check.FullName
    if ($LASTEXITCODE -ne 0) { throw "$($check.Name) failed" }
  }
}

# Runs everything CI checks, in order, and stops at the first failure.
function Invoke-Verify {
  $script:Deps = [switch]$true
  Invoke-Check
  Step "pnpm audit" { pnpm audit --prod }
  Invoke-Test
  Step "pnpm test" { pnpm test }
  Write-Host "verify passed." -ForegroundColor Green
}

# Starts `dev` in a hidden PowerShell with its output in the scratch log.
function Start-Live {
  $log = Join-Path $scratch "dev.log"
  $cmd = "& '$PSCommandPath' dev *> '$log'"
  Start-Process powershell -WindowStyle Hidden -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", $cmd
  Write-Host "live: tauri dev started, log $log; scripts\cue.ps1 stop ends it"
}

# Measures the release exe and prints the result (from -Json when given, else bench/latest.json) next to bench/baseline.json.
function Invoke-Bench {
  Need-Release
  Invoke-Harness "verify/bench.ps1"
  $json = (ConvertTo-Splat $Rest)["Json"]
  if ($json) { & (Join-Path $PSScriptRoot "verify/bench-report.ps1") -Result $json } else { & (Join-Path $PSScriptRoot "verify/bench-report.ps1") }
}

$tools = "rust", "node", "pnpm", "vs"
$front = "node", "pnpm"
$modes = [ordered]@{
  "help" = @{ Help = "this list"; Run = { Write-Usage; exit 0 } }
  "setup" = @{ Needs = $tools + "webview"; NoEnv = $true; Help = "check every tool Cue needs, list what is missing, install the frontend packages"; Run = { Write-Host "Ready. Next: scripts\cue.ps1 dev (run with hot reload) or scripts\cue.ps1 verify (everything CI checks)." -ForegroundColor Green; exit 0 } }
  "dev" = @{ Needs = $tools + "webview"; Help = "run the app with hot reload"; Run = { Stop-Vite; try { Step "pnpm tauri dev" { pnpm tauri dev } } finally { Stop-Vite } } }
  "live" = @{ Needs = $tools + "webview"; NoEnv = $true; Help = "dev, hidden, log in $scratch\dev.log; stop ends it"; Run = { Start-Live; exit 0 } }
  "stop" = @{ Help = "end every Cue process and the Vite server"; Run = { cmd /c "taskkill /F /IM cue.exe >nul 2>&1"; Stop-Vite; exit 0 } }
  "verify" = @{ Needs = $tools + "deptools"; Help = "everything CI checks, in one command: check -Deps, audit, test, test-frontend. Run it before a pull request"; Run = { Invoke-Verify } }
  "check" = @{ Needs = $tools; Help = "types, ESLint, theme contrast, rustfmt, clippy (-Quick: types and cargo check only; -Deps: also unused crates and advisories)"; Run = { Invoke-Check } }
  "check-frontend" = @{ Needs = $front; Help = "types, ESLint and theme contrast only (also runs on Linux)"; Run = { Invoke-FrontendCheck } }
  "deps" = @{ Needs = @("deptools"); Help = "unused crates, advisories, bans and sources (cargo machete and cargo deny)"; Run = { Invoke-Deps } }
  "test" = @{ Needs = "rust", "vs"; Help = "cargo test and the script self-checks"; Run = { Invoke-Test } }
  "test-frontend" = @{ Needs = $front; Help = "frontend tests (also runs on Linux)"; Run = { Step "pnpm test" { pnpm test } } }
  "audit" = @{ Needs = $front; Help = "pnpm audit --prod"; Run = { Step "pnpm audit" { pnpm audit --prod } } }
  "release" = @{ Needs = $tools; Profile = "release"; Help = "optimized exe only"; Run = { Step "pnpm tauri build --no-bundle" { pnpm tauri build --no-bundle } } }
  "build" = @{ Needs = $tools; Profile = "release"; Help = "exe and installer"; Run = { Step "pnpm tauri build" { pnpm tauri build } } }
  "run" = @{ Help = "start the release exe"; Run = { Need-Release; Start-Process (Resolve-Path $release).Path; exit 0 } }
  "lifecycle" = @{ Help = "release exe starts, opens and closes settings, leaves no orphan process (build it first)"; Run = { Need-Release; Invoke-Harness "verify/lifecycle.ps1" } }
  "bench" = @{ Help = "measure the release exe, then print bench/baseline.json next to the result (no pass or fail)"; Run = { Invoke-Bench } }
  "smoke" = @{ Help = "the hotkey path: overlay and microphone while the key is held (the end-to-end checks need an unlocked desktop)"; Run = { Need-Release; Invoke-Harness "verify/smoke.ps1" } }
  "threads" = @{ Help = "which thread burns CPU at idle and the module each thread starts in"; Run = { Need-Release; Invoke-Harness "verify/threads.ps1" } }
  "selection" = @{ Help = "the selection shortcut rewrites a Cue Note"; Run = { Need-Release; Invoke-Harness "verify/selection-check.ps1" } }
  "dictionary-check" = @{ Help = "the dictionary Card and Search windows: time, memory, screenshots"; Run = { Need-Release; Invoke-Harness "verify/dictionary-check.ps1" } }
  "dictionary-notes" = @{ Help = "a dictionary lookup from a Cue Note"; Run = { Need-Release; Invoke-Harness "verify/dictionary-notes-check.ps1" } }
  "dictionary-browser" = @{ Help = "a dictionary lookup from a browser page"; Run = { Need-Release; Invoke-Harness "verify/dictionary-browser-check.ps1" } }
  "text-access" = @{ Help = "selection and dictionary in Edge, an edit control and Notepad"; Run = { Need-Release; Invoke-Harness "verify/text-access-check.ps1" } }
  "selection-repeat" = @{ Help = "press the Transform Text shortcut N times (-Presses, default 50) into Notepad and Edge and count misses by reason from cue.log (no pass or fail)"; Run = { Need-Release; Invoke-Harness "verify/selection-repeat.ps1" } }
  "soak" = @{ Help = "open and close Grab Text 100 times, no handle or GUI object leak"; Run = { Need-Release; Invoke-Harness "verify/soak.ps1" } }
  "leak" = @{ Help = "the dictation shortcut never types its own key into the focused app"; Run = { Need-Release; Invoke-Harness "verify/leak-check.ps1" } }
  "tray-release" = @{ Help = "the tray menu opens fast, stays open and looks right, on the release exe"; Run = { Need-Release; Invoke-Harness "verify/tray-smoke.ps1" @{ exe = (Resolve-Path $release).Path; shot = (Join-Path $scratch "menu.png") } } }
  "smoke-debug" = @{ Needs = $tools; Help = "smoke on a debug build (built first)"; Run = { Step "cargo build" { cargo build --manifest-path $manifest }; Invoke-Harness "verify/smoke.ps1" @{ exe = (Resolve-Path $debug).Path } } }
  "tray" = @{ Needs = $tools; Help = "the tray menu on a debug build (built first)"; Run = { Step "cargo build" { cargo build --manifest-path $manifest }; Invoke-Harness "verify/tray-smoke.ps1" @{ shot = (Join-Path $scratch "menu.png") } } }
  "insert" = @{ Needs = $tools; Help = "text insertion into Notepad, typed and pasted, on a debug build (built first)"; Run = { Step "cargo build" { cargo build --manifest-path $manifest }; Invoke-Harness "verify/insert-check.ps1" } }
  "installer" = @{ Help = "install, run and uninstall the built installer; it deletes Cue's install and data, so use a VM or clean machine (-Force overrides)"; Run = { Invoke-Harness "verify/installer-check.ps1" } }
  "lab" = @{ Help = "lab <name>: maintainer experiments: models speech corpus textmodels texttasks textgolden textlatency textsecurity"; Run = { & (Join-Path $PSScriptRoot "lab/lab.ps1") -Name $Rest[0] -Rest ($Rest | Select-Object -Skip 1) -Scratch $scratch; if ($LASTEXITCODE) { exit $LASTEXITCODE } } }
}

# Prints the usage line and one line per mode from the mode table.
function Write-Usage {
  Write-Host "Cue build tool. Usage: powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\cue.ps1 <mode> [-Quick] [-Deps] [harness arguments]"
  foreach ($k in $modes.Keys) { Write-Host ("  {0,-19} {1}" -f $k, $modes[$k].Help) }
}

$m = $modes[$Mode]
if (-not $m) { Write-Usage; exit 2 }
$want = @($m.Needs | Where-Object { $_ })
if ($Mode -eq "check" -and $Deps) { $want += "deptools" }
if ($want) { Test-Tools $want }
if ($want -contains "pnpm") { Install-Packages }
if (-not $m.NoEnv -and $want -contains "vs") { Import-VsEnv }
if (-not $m.NoEnv -and $want -contains "rust") { Use-SharedBuild "main" $(if ($m.Profile) { $m.Profile } else { "dev" }) $manifest | Out-Null }
& $m.Run
