# File: scripts/verify/upgrade-check.ps1
# Purpose: prove the upgrade path from published 0.3.x releases. For each old version: download its installer (through scripts/ci/retry.ps1), install it silently for the current user, seed settings.json and a model-sized file, run the new installer over it as the updater does (/S /UPDATE), then check the version, the seeded data (byte for byte) and that the app reaches the tray.
# Contents: Get-InstallDir (the folder the installer registered), Clear-Cue, Invoke-Setup, Install-Release (download and install one old release), the per-version loop, the final uninstall.
# shortcut: the in-app update click (download, verify, CreateProcessW) is not driven, only the installer it launches; drive it when a test release feed exists. Only the releases named in -From are tried.
# It installs and uninstalls Cue and deletes the Cue data folders, so it refuses to run when Cue is installed or has data unless -Force is given; installer-check.ps1 calls it after its own clean uninstall.
# Usage: pwsh scripts/verify/upgrade-check.ps1 -Installer path [-From 0.3.0,0.3.4] [-Force]
param([Parameter(Mandatory)][string]$Installer, [ValidatePattern('^\d+\.\d+\.\d+$')][string[]]$From = @("0.3.0", "0.3.1", "0.3.2", "0.3.3", "0.3.4"), [switch]$Force)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
$default = Join-Path $env:LOCALAPPDATA "Cue"
$uninstKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Cue"
$data = Join-Path $env:APPDATA "dev.cue.app"
$settings = Join-Path $data "settings.json"
$model = Join-Path $data "models\seed\model.bin"
$new = ([regex]::Match((Split-Path $Installer -Leaf), '_(\d+\.\d+\.\d+)_')).Groups[1].Value
if (-not $new) { throw "Cannot read the version from the installer name: $Installer" }

$occupied = @($default, $data, "$env:LOCALAPPDATA\dev.cue.app", $uninstKey) | Where-Object { Test-Path $_ }
if ($occupied -and -not $Force) { throw "Cue is already installed or has data here ($($occupied -join ', ')). This check installs and uninstalls Cue and deletes those folders. Run it on a clean machine or a VM, or pass -Force if you accept losing them." }

# Returns the folder the installer registered for Cue, or the default folder when none is registered.
function Get-InstallDir {
  $location = (Get-ItemProperty $uninstKey -ErrorAction SilentlyContinue).InstallLocation
  if ($location) { return $location.Trim('"') }
  return $default
}

# Stops Cue and removes the install and data so the next version starts clean.
function Clear-Cue {
  $install = Get-InstallDir
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  $null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue
  if (Test-Path (Join-Path $install "uninstall.exe")) { Invoke-Setup (Join-Path $install "uninstall.exe") "/S" }
  $null = WaitUntil { -not (Test-Path (Join-Path $install "cue.exe")) } 30000
  Remove-Item $data, "$env:LOCALAPPDATA\dev.cue.app" -Recurse -Force -ErrorAction SilentlyContinue
  if ((Test-Path (Join-Path $install "cue.exe")) -or (Test-Path $data)) { throw "Cleanup left Cue behind in $install or $data" }
}

# Runs an installer or uninstaller with the given arguments, waits, and throws on a non-zero exit code. Input: exe path, arguments.
function Invoke-Setup($exe, $setupArgs) {
  $p = Start-Process $exe -ArgumentList $setupArgs -Wait -PassThru
  if ($p.ExitCode -ne 0) { throw "$exe $setupArgs exited with code $($p.ExitCode)" }
}

# Downloads the published installer of one version with retries and installs it for the current user. Input: version.
function Install-Release($version) {
  $file = Join-Path $env:TEMP "Cue_${version}_x64-setup.exe"
  $url = "https://github.com/Rishik15/cue/releases/download/v$version/Cue_${version}_x64-setup.exe"
  & (Join-Path $PSScriptRoot "..\ci\retry.ps1") "curl.exe -fsSL -o '$($file.Replace("'", "''"))' '$url'"
  if ($LASTEXITCODE -ne 0) { throw "Could not download $url" }
  Invoke-Setup $file "/S /CURRENTUSER"
  Remove-Item $file -Force
}

Clear-Cue
try {
  foreach ($old in $From) {
    "upgrade $old -> $new"
    Install-Release $old
    Check "$old installed" ((Get-ItemProperty $uninstKey -ErrorAction SilentlyContinue).DisplayVersion -eq $old)
    New-Item -ItemType Directory (Split-Path $model) -Force | Out-Null
    Set-Content $settings '{"onboarded":true,"check_updates":false}' -NoNewline
    [IO.File]::WriteAllBytes($model, [byte[]](1..255))
    $seeded = (Get-FileHash $settings).Hash, (Get-FileHash $model).Hash
    Invoke-Setup $Installer "/S /UPDATE"
    Check "$old -> $new version is the new one" ((Get-ItemProperty $uninstKey).DisplayVersion -eq $new)
    Check "$old -> $new settings kept" ((Test-Path $settings) -and (Get-FileHash $settings).Hash -eq $seeded[0])
    Check "$old -> $new model file kept" ((Test-Path $model) -and (Get-FileHash $model).Hash -eq $seeded[1])
    $install = Get-InstallDir
    "installed at $install"
    $app = Start-Process (Join-Path $install "cue.exe") -ArgumentList "--background" -PassThru
    Check "$old -> $new app reaches the tray" ($null -ne (WaitUntil { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000))
    Stop-Process -Id $app.Id -Force
    Clear-Cue
  }
} finally { Clear-Cue }
Complete-Checks
