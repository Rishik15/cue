# Purpose: prove the dictation shortcut never types its own key into the focused app. Ctrl+Space is held, Ctrl is let go first (the usual way to
# end a hold), Space keeps auto-repeating for a moment, then is released; Notepad must stay empty.
# Needs a release build first (scripts/winsync release). Usage: pwsh scripts/leak-check.ps1 [-Exe path]
param([string]$Exe = (Join-Path $PSScriptRoot "..\src-tauri\target\release\cue.exe"))
$ErrorActionPreference = "Stop"
Add-Type @'
using System; using System.Runtime.InteropServices;
public class K { [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra); }
'@
function Key($vk, $scan, $up) { [K]::keybd_event($vk, $scan, $(if ($up) { 2 } else { 0 }), [UIntPtr]::Zero) }
Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
$app = Start-Process $Exe -ArgumentList "--background" -PassThru
Start-Sleep 3
$file = Join-Path $env:TEMP "cue-leak-test.txt"; Set-Content $file "" -NoNewline
try {
  Start-Process notepad -ArgumentList $file; Start-Sleep 6
  (New-Object -ComObject WScript.Shell).AppActivate("cue-leak-test") | Out-Null; Start-Sleep 1
  Key 0xA2 0x1D $false; Key 0x20 0x39 $false          # Ctrl down, Space down
  Start-Sleep -Milliseconds 400
  Key 0xA2 0x1D $true                                  # Ctrl up first
  1..8 | ForEach-Object { Start-Sleep -Milliseconds 40; Key 0x20 0x39 $false }   # Space auto-repeats
  Key 0x20 0x39 $true                                  # Space up
  Start-Sleep 2
  Key 0xA2 0x1D $false; Key 0x53 0x1F $false; Key 0x53 0x1F $true; Key 0xA2 0x1D $true   # Ctrl+S
  Start-Sleep -Milliseconds 1500
  $text = Get-Content $file -Raw
  "file holds: [$text]"
  if ([string]::IsNullOrEmpty($text)) { "PASS" } else { "FAIL"; exit 1 }
} finally {
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
}
