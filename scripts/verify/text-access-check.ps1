# File: scripts/verify/text-access-check.ps1
# Purpose: prove the one shared selection path in other people's apps on Windows, with no model: in Edge (an editable page) and in a classic Windows edit control, a Selection rule (clean links, then uppercase) rewrites the selected text through the transform strip (shortcut Left Ctrl + Left Alt + Space, key 1, Enter) and the app must hold the rewritten text; and in both the dictionary shortcut (Left Ctrl + Left Alt + D) must open the Card for the last word of a sentence. Your rules.json, settings.json, notes and looked-up words are set aside and restored; so is the clipboard.
# Contents: Select-LastWord, Press-Selection, Read-App, Open-Host, Open-Edge, Run-Transform, Run-Dictionary and the cases.
# Needs a release build, Edge; the dictionary cases need the English dictionary Pack and are skipped without it. Usage: pwsh scripts/verify/text-access-check.ps1 [-Exe path]
param(
  [string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"),
  [string]$Browser = "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe"
)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
. (Join-Path $PSScriptRoot "..\lib\require-unlocked.ps1")
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\userdata.ps1")
Add-Type -AssemblyName System.Windows.Forms
$pack = Join-Path $env:LOCALAPPDATA "dev.cue.app\models\dictionary\dictionary-en\dictionary.db"
$rules = Join-Path $dir "rules.json"; $backup = Join-Path $env:TEMP "cue-rules-backup.json"
$sample = "see https://a.com/p?id=1&utm_source=x now"
$page = Join-Path $env:TEMP "cue-text-access.html"
$profile = Join-Path $env:TEMP "cue-text-access-profile"

# Selects the last word of the line the caret is in: End, then Ctrl+Shift+Left (raw scan codes, like a person's keys).
function Select-LastWord {
  [Win32]::keybd_event(0x23, 0x4F, 1, [UIntPtr]::Zero); [Win32]::keybd_event(0x23, 0x4F, 3, [UIntPtr]::Zero)
  foreach ($m in @($LCTRL, @(0xA0, 0x2A))) { [Win32]::keybd_event($m[0], $m[1], 0, [UIntPtr]::Zero) }
  [Win32]::keybd_event(0x25, 0x4B, 1, [UIntPtr]::Zero); [Win32]::keybd_event(0x25, 0x4B, 3, [UIntPtr]::Zero)
  foreach ($m in @(@(0xA0, 0x2A), $LCTRL)) { [Win32]::keybd_event($m[0], $m[1], 2, [UIntPtr]::Zero) }
  Start-Sleep -Milliseconds 300
}
# Reads the app's whole text by copying it (Ctrl+A, Ctrl+C) until it differs from `$not` and two reads in a row agree. Changes the clipboard. Inputs: a text it must differ from, timeout in ms.
function Read-App($not, $ms) {
  $script:prev = $null
  $null = WaitUntil {
    Chord @(,$LCTRL) 0x41 0x1E; Chord @(,$LCTRL) 0x43 0x2E; Start-Sleep -Milliseconds 400
    $now = [System.Windows.Forms.Clipboard]::GetText()
    $done = ($now -ne $not) -and ($now -ceq $script:prev); $script:prev = $now; $done
  } $ms
  [System.Windows.Forms.Clipboard]::GetText().Trim()
}
# Opens a window with a classic Windows edit control (a multiline WinForms TextBox, run from a renamed copy of PowerShell because Cue treats powershell.exe as a terminal: the EDIT class every old app uses) holding `$text`, with the keyboard. Input: the text.
function Open-Host($text) {
  $env:CUE_HOST_TEXT = $text
  $form = 'Add-Type -AssemblyName System.Windows.Forms; $f = New-Object Windows.Forms.Form; $f.Text = "Cue Edit Host"; $f.Width = 1100; $f.Height = 300; $t = New-Object Windows.Forms.TextBox; $t.Multiline = $true; $t.Dock = "Fill"; $t.Font = New-Object Drawing.Font("Segoe UI", 20); $t.Text = $env:CUE_HOST_TEXT; $t.Add_KeyDown({ if ($_.Control -and $_.KeyCode -eq "A") { $t.SelectAll(); $_.SuppressKeyPress = $true } }); $f.Controls.Add($t); $f.Add_Shown({ $f.Activate(); $t.Focus(); $t.Select($t.Text.Length, 0) }); [Windows.Forms.Application]::Run($f)'
  $exe = Join-Path $env:TEMP "cue-edit-host.exe"
  Copy-Item "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" $exe -Force
  Start-Process $exe -ArgumentList "-NoProfile", "-EncodedCommand", ([Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($form)))
  $null = Await "the edit host window" { (Win $null "Cue Edit Host") -ne [IntPtr]::Zero } 20000
  $null = Await "the edit host to take the keyboard" { $null = (New-Object -ComObject WScript.Shell).AppActivate("Cue Edit Host"); $h = Win $null "Cue Edit Host"; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 20000
  Start-Sleep -Milliseconds 1500
}
# Opens Edge on an editable page holding `$text`, with the keyboard. Input: the text.
function Open-Edge($text) {
  [IO.File]::WriteAllText($page, "<title>Cue Text Access</title><body style='font:40px sans-serif'><div contenteditable id=e>$([Net.WebUtility]::HtmlEncode($text))</div><script>e.focus()</script>")
  $script:edge = Start-Process $Browser -ArgumentList "--user-data-dir=$profile", "--no-first-run", "--app=file:///$($page -replace '\\', '/')" -PassThru
  $null = Await "Edge to take the keyboard" { $h = Win $null "Cue Text Access"; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 20000
  Start-Sleep -Milliseconds 1500
}
# Closes the edit host and Edge windows the cases opened.
function Close-Apps {
  Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like "*cue-text-access-profile*" -or $_.Name -eq "cue-edit-host.exe" } | ForEach-Object { Stop-Process $_.ProcessId -Force -ErrorAction SilentlyContinue }
  Start-Sleep -Milliseconds 500
}
# Presses the selection shortcut (Left Ctrl + Left Alt + Space) and keeps it down until the strip is visible, as a held shortcut is while Cue copies; throws when the strip never shows.
function Press-Selection {
  foreach ($m in @($LCTRL, $LALT)) { [Win32]::keybd_event($m[0], $m[1], 0, [UIntPtr]::Zero) }
  [Win32]::keybd_event(0x20, 0x39, 0, [UIntPtr]::Zero)
  $shown = WaitUntil { Visible $null "Cue Transform Text" } 8000
  [Win32]::keybd_event(0x20, 0x39, 2, [UIntPtr]::Zero)
  foreach ($m in @($LALT, $LCTRL)) { [Win32]::keybd_event($m[0], $m[1], 2, [UIntPtr]::Zero) }
  if ($null -eq $shown) { throw "The transform strip was not shown 8 s after the selection shortcut." }
}
# Rewrites the selected text of the app through the strip and checks the app holds it. Inputs: label, script block that opens the app with `$sample`, window class or title lookup (`$class`, `$title`) of the app.
function Run-Transform($label, $open, $class, $title) {
  & $open $sample
  Chord @(,$LCTRL) 0x41 0x1E
  [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  $mark = Get-LogMark
  Press-Selection
  $null = Await "the strip to take the keyboard" { $h = Win $null "Cue Transform Text"; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 15000
  Start-Sleep -Milliseconds 500
  Chord @() 0x31 0x02; Start-Sleep 2
  Chord @() 0x0D 0x1C; Start-Sleep 2
  $null = Await "$label to hold the keyboard again" { $h = Win $class $title; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 10000
  $kept = [System.Windows.Forms.Clipboard]::GetText() -eq "ORIGINAL CLIPBOARD"
  $text = Read-App $sample 15000
  "$label log: " + (Read-LogSince $mark); "$label text: [$text]"
  Check "$label transform: the app holds the rewritten text" ($text -ieq "SEE HTTPS://A.COM/P?ID=1 NOW")
  Check "$label transform: clipboard left as it was" $kept
  Close-Apps
}
# Looks up the last word of a sentence in the app and checks the Card opens. Inputs: label, script block that opens the app with a text.
function Run-Dictionary($label, $open) {
  & $open "I drank some water"
  Select-LastWord
  [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  $mark = Get-LogMark
  Chord @($LCTRL, $LALT) 0x44 0x20
  $shown = WaitUntil { Visible $null "Cue Dictionary" } 5000
  "$label log: " + (Read-LogSince $mark)
  Check "$label dictionary: the Card opens for the selected word" ($null -ne $shown -and (Read-LogSince $mark).Contains("dictionary: decided card"))
  Close-Apps
}

if (Test-Path $settings) { Move-Item $settings $settingsBackup -Force }
Set-AsideNotes
New-Item -ItemType Directory -Force $dir | Out-Null
if (Test-Path $rules) { Copy-Item $rules $backup -Force }
$clipboard = [System.Windows.Forms.Clipboard]::GetText()
try {
  Set-Content $settings '{"onboarded":true}'
  '{ "version": 1, "rules": [ { "name": "check", "trigger": "selection", "actions": ["clean_links", "upper_case"], "output": "insert" } ] }' | Set-Content $rules
  $mark = Get-LogMark
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  Run-Transform "edit   " ${function:Open-Host} $null "Cue Edit Host"
  Run-Transform "edge   " ${function:Open-Edge} $null "Cue Text Access"
  if (Test-Path $pack) {
    Run-Dictionary "edit   " ${function:Open-Host}
    Run-Dictionary "edge   " ${function:Open-Edge}
  } else { "SKIPPED the dictionary cases: no dictionary Pack is installed ($pack not found)." }
} finally {
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  Close-Apps
  Restore-Settings
  Restore-Notes
  if (Test-Path $backup) { Copy-Item $backup $rules -Force; Remove-Item $backup } else { Remove-Item $rules -ErrorAction SilentlyContinue }
  if ($clipboard) { [System.Windows.Forms.Clipboard]::SetText($clipboard) }
}
Complete-Checks
