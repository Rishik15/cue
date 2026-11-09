# File: scripts/verify/installer-check.ps1
# Purpose: verify the installer's footprint. Silently installs the installer for the current user (/CURRENTUSER; the installer also offers all users), checks the program files, uninstall entry and Start menu shortcut exist, starts the installed app, then silently uninstalls and checks everything it created outside the data directories is gone. The uninstaller removes the data folders too (not during an update).
# Contents: the install, start and uninstall steps with state waits, the verdict.
# It deletes the Cue install and the Cue data folders of the current user, so it refuses to run when either exists unless -Force is given: use a clean machine, a VM or a throwaway account.
# Usage: pwsh scripts/verify/installer-check.ps1 [-Installer path] [-Force]   (build first: pnpm tauri build)
param([string]$Installer = (Get-ChildItem (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\bundle\nsis\*-setup.exe") | Sort-Object LastWriteTime | Select-Object -Last 1).FullName, [switch]$Force)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
$install = Join-Path $env:LOCALAPPDATA "Cue"
$uninstKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Cue"
$runKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$publisherKey = "HKCU:\Software\Rishik\Cue"
$startMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Cue.lnk"

$occupied = @("$env:LOCALAPPDATA\Cue", "$env:APPDATA\dev.cue.app", "$env:LOCALAPPDATA\dev.cue.app") | Where-Object { Test-Path $_ }
if ($occupied -and -not $Force) { throw "Cue is already installed or has data here ($($occupied -join ', ')). This check installs and uninstalls Cue and deletes those folders. Run it on a clean machine or a VM, or pass -Force if you accept losing them." }

Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
$null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue
"installer: $Installer ($([math]::Round((Get-Item $Installer).Length / 1MB, 2)) MB)"
Start-Process $Installer -ArgumentList "/S", "/CURRENTUSER" -Wait
$location = (Get-ItemProperty $uninstKey -ErrorAction SilentlyContinue).InstallLocation
if ($location) { $install = $location.Trim('"') }
Check "program files installed" (Test-Path (Join-Path $install "cue.exe"))
Check "uninstall entry registered" (Test-Path $uninstKey)
Check "Start menu shortcut created" (Test-Path $startMenu)

$app = Start-Process (Join-Path $install "cue.exe") -ArgumentList "--background" -PassThru
$reached = $null -ne (WaitUntil { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000)
Check "installed app reaches the tray" $reached
Stop-Process -Id $app.Id -Force
$null = Wait-Process -Id $app.Id -Timeout 10 -ErrorAction SilentlyContinue

Start-Process (Join-Path $install "uninstall.exe") -ArgumentList "/S" -Wait
$null = WaitUntil { -not (Test-Path (Join-Path $install "cue.exe")) -and -not (Test-Path $uninstKey) -and -not (Test-Path "$env:APPDATA\dev.cue.app") } 30000
Check "program files removed" (-not (Test-Path (Join-Path $install "cue.exe")))
Check "uninstall entry removed" (-not (Test-Path $uninstKey))
Check "Start menu shortcut removed" (-not (Test-Path $startMenu))
Check "installer registry key removed" (-not (Test-Path $publisherKey))
Check "launch-at-login entry removed" ($null -eq (Get-ItemProperty $runKey -ErrorAction SilentlyContinue).Cue)
Check "app data removed" (-not (Test-Path "$env:APPDATA\dev.cue.app") -and -not (Test-Path "$env:LOCALAPPDATA\dev.cue.app"))
Complete-Checks
