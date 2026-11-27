# File: scripts/verify/dictionary-notes-check.ps1
# Purpose: prove the dictionary lookup works from inside a Cue Note: a sentence is pasted into the note, its last word selected, the lookup shortcut (Left Ctrl + Left Alt + D) is pressed, and Cue must open the Card for it. Your settings, notes and looked-up words are set aside and restored; so is the clipboard's text, and a clipboard holding more than plain text is refused before anything changes.
# Contents: the case.
# Needs a release build and the English dictionary Pack installed. Usage: pwsh scripts/verify/dictionary-notes-check.ps1 [-Exe path]
param([string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"))
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\keys.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
. (Join-Path $PSScriptRoot "..\lib\require-unlocked.ps1")
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\userdata.ps1")
. (Join-Path $PSScriptRoot "..\lib\clipformats.ps1")
. (Join-Path $PSScriptRoot "..\lib\note.ps1")
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
  Fill-Note $mark "I drank some water"
  Tap 0x23 0x4F $true
  Chord @($LCTRL, $LSHIFT) 0x25 0x4B $true
  $seq = [Win32]::GetClipboardSequenceNumber(); Chord @(,$LCTRL) 0x43 0x2E; Wait-Copied $seq
  "selected: [" + [System.Windows.Forms.Clipboard]::GetText() + "]"
  [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  $mark = Get-LogMark
  Chord @($LCTRL, $LALT) 0x44 0x20
  $shown = WaitUntil { Visible $app.Id $null "Cue Dictionary" } 5000
  "log: " + (Read-LogSince $mark)
  Check "the Card opens for a word selected in a Cue Note" ($null -ne $shown -and (Test-LogLine $mark "dictionary_decided" "card"))
} finally {
  try { Exit-UserData } finally {
  if ($clipboard) { [System.Windows.Forms.Clipboard]::SetText($clipboard) } else { [System.Windows.Forms.Clipboard]::Clear() }
  }
}
Complete-Checks
