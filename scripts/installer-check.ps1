# Purpose: verify the installer's footprint. Silently installs the per-user installer, checks the program files, uninstall
# entry and Start menu shortcut exist, starts the installed app, then silently uninstalls and checks everything it created
# outside the data directories is gone. Settings and web view data are removed only by the uninstaller's "Delete the
# application data" checkbox or the app's Remove All Cue Data action, so they are reported, not asserted.
# Usage: pwsh scripts/installer-check.ps1 [-Installer path]   (build first: pnpm tauri build)
param([string]$Installer = (Get-ChildItem (Join-Path $PSScriptRoot "..\src-tauri\target\release\bundle\nsis\*-setup.exe") | Sort-Object LastWriteTime | Select-Object -Last 1).FullName)
$ErrorActionPreference = "Stop"
Add-Type @'
using System; using System.Runtime.InteropServices;
public class I { [DllImport("user32.dll")] public static extern IntPtr FindWindow(string cls, string title); }
'@
$install = Join-Path $env:LOCALAPPDATA "Cue"
$uninstKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Cue"
$runKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$startMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Cue.lnk"
$checks = [ordered]@{}
function Check($name, $ok) { $script:checks[$name] = [bool]$ok; "{0,-48} {1}" -f $name, $(if ($ok) { "ok" } else { "FAILED" }) }

Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
"installer: $Installer ($([math]::Round((Get-Item $Installer).Length / 1MB, 2)) MB)"
Start-Process $Installer -ArgumentList "/S" -Wait
Check "program files installed" (Test-Path (Join-Path $install "cue.exe"))
Check "uninstall entry registered" (Test-Path $uninstKey)
Check "Start menu shortcut created" (Test-Path $startMenu)

$app = Start-Process (Join-Path $install "cue.exe") -ArgumentList "--background" -PassThru
$sw = [Diagnostics.Stopwatch]::StartNew()
while ([I]::FindWindow("tray_icon_app", [NullString]::Value) -eq [IntPtr]::Zero -and $sw.ElapsedMilliseconds -lt 10000) { Start-Sleep -Milliseconds 50 }
Check "installed app reaches the tray" ([I]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero)
Stop-Process -Id $app.Id -Force
Start-Sleep 1

Start-Process (Join-Path $install "uninstall.exe") -ArgumentList "/S" -Wait
Start-Sleep 2
Check "program files removed" (-not (Test-Path (Join-Path $install "cue.exe")))
Check "uninstall entry removed" (-not (Test-Path $uninstKey))
Check "Start menu shortcut removed" (-not (Test-Path $startMenu))
Check "launch-at-login entry removed" ($null -eq (Get-ItemProperty $runKey -ErrorAction SilentlyContinue).Cue)
"left behind (data, by design): " + ((@("$env:APPDATA\dev.cue.app", "$env:LOCALAPPDATA\dev.cue.app") | Where-Object { Test-Path $_ }) -join ", ")
if ($checks.Values -contains $false) { "FAIL"; exit 1 } else { "PASS" }
