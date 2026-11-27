# File: scripts/verify/dictionary-browser-check.ps1
# Purpose: prove the dictionary lookup works in a browser's editable page: a page with an editable line is opened in Edge (or the browser named by -Browser), its last word selected, the lookup shortcut (Left Ctrl + Left Alt + D) is pressed, and Cue must open the Card for it. Your settings, notes and looked-up words are set aside and restored; so is the clipboard's text, and a clipboard holding more than plain text is refused before anything changes.
# Contents: the case.
# Needs a release build and the English dictionary Pack installed. Usage: pwsh scripts/verify/dictionary-browser-check.ps1 [-Exe path] [-Browser path]
param(
  [string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"),
  [string]$Browser = "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe"
)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\keys.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
. (Join-Path $PSScriptRoot "..\lib\require-unlocked.ps1")
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\userdata.ps1")
. (Join-Path $PSScriptRoot "..\lib\clipformats.ps1")
. (Join-Path $PSScriptRoot "..\lib\edge.ps1")
Add-Type -AssemblyName System.Windows.Forms
$pack = Join-Path $env:LOCALAPPDATA "dev.cue.app\models\dictionary\dictionary-en\dictionary.db"
if (-not (Test-Path $pack)) { "SKIPPED: no dictionary Pack is installed ($pack not found); install the English Pack in Cue's settings and run this again."; exit 0 }

Assert-TextClipboard
$clipboard = [System.Windows.Forms.Clipboard]::GetText()
Enter-UserData
try {
  New-Item -ItemType Directory -Force $dir | Out-Null
  Set-Content $settings '{"onboarded":true}'
  $mark = Get-LogMark
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  $file = Join-Path $env:TEMP "cue-browser-check.html"
  Set-Content $file "<title>Cue Browser Check</title><body style='font:40px sans-serif'><div contenteditable id=e>I drank some water</div><script>e.focus()</script>"
  $page = "file:///" + ($file -replace "\\", "/")
  $profile = Join-Path $env:TEMP "cue-browser-check-profile"
  $b = Start-Process $Browser -ArgumentList "--user-data-dir=$profile", "--no-first-run", "--app=$page" -PassThru
  $null = Await "the browser window to take the keyboard" { $h = Win (Get-EdgeIds $profile) $null "Cue Browser Check"; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 20000
  Start-Sleep -Milliseconds 1500
  Tap 0x23 0x4F $true
  Chord @($LCTRL, $LSHIFT) 0x25 0x4B $true
  Start-Sleep -Milliseconds 300
  Chord @(,$LCTRL) 0x43 0x2E
  Start-Sleep -Milliseconds 300
  "selected: [" + [System.Windows.Forms.Clipboard]::GetText() + "]"
  [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  $mark = Get-LogMark
  Chord @($LCTRL, $LALT) 0x44 0x20
  $shown = WaitUntil { Visible $app.Id $null "Cue Dictionary" } 5000
  "log: " + (Read-LogSince $mark)
  Check "the Card opens for a word selected in the browser" ($null -ne $shown -and (Read-LogSince $mark).Contains("dictionary: decided card"))
} finally {
  try {
    if ($b) { Stop-Process $b -Force -ErrorAction SilentlyContinue; Get-EdgeIds $profile | ForEach-Object { Stop-Process $_ -Force -ErrorAction SilentlyContinue } }
  } finally {
    try { Exit-UserData } finally {
      if ($clipboard) { [System.Windows.Forms.Clipboard]::SetText($clipboard) } else { [System.Windows.Forms.Clipboard]::Clear() }
    }
  }
}
Complete-Checks
