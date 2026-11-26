# File: scripts/verify/text-access-check.ps1
# Purpose: prove the one shared selection path in other people's apps on Windows, with no model: in Edge (an editable page) and in a classic Windows edit control, a Selection rule (clean links, then uppercase) rewrites the selected text through the transform strip (shortcut Left Ctrl + Left Alt + Space, key 1, Enter) and the app must hold the rewritten text; and in both the dictionary shortcut (Left Ctrl + Left Alt + D) must open the Card for the last word of a sentence. Your rules.json, settings.json, notes and looked-up words are set aside and restored; so is the clipboard's text (a non-text clipboard is not restored).
# Contents: Protected, Press-SelectLastWord, Select-LastWord, Focused-Text, Press-Selection, Read-App, Open-Host, Open-Edge, Run-Transform, Run-Dictionary and the cases.
# Needs a release build, Edge; the dictionary cases need the English dictionary Pack and are skipped without it. Usage: pwsh scripts/verify/text-access-check.ps1 [-Exe path]
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
. (Join-Path $PSScriptRoot "..\lib\notepad.ps1")
Add-Type -AssemblyName System.Windows.Forms, UIAutomationClient, UIAutomationTypes
$pack = Join-Path $env:LOCALAPPDATA "dev.cue.app\models\dictionary\dictionary-en\dictionary.db"
$rules = Join-Path $dir "rules.json"
$sample = "see https://a.com/p?id=1&utm_source=x now"
$page = Join-Path $env:TEMP "cue-text-access.html"
$profile = Join-Path $env:TEMP "cue-text-access-profile"
$notepadPart = "cue-text-access-notepad-" + [guid]::NewGuid()
$notepadFile = Join-Path $env:TEMP "$notepadPart.txt"
$script:ownedNotepad = @()

# Runs one cleanup step and reports a failure instead of stopping, so one failed step cannot skip the restores after it. Inputs: what the step does, the step.
function Protected($what, $step) { try { & $step } catch { "WARNING: could not $what`: $_" } }
# Presses End, then Ctrl+Shift+Left (raw scan codes, like a person's keys): the last word of the line is selected.
function Press-SelectLastWord {
  Tap 0x23 0x4F $true
  Chord @($LCTRL, $LSHIFT) 0x25 0x4B $true
}
# Selects the last word of the line the caret is in and waits until a copy shows a selection exists; the selecting is repeated while it does (an app may lose the first keys). Changes the clipboard.
function Select-LastWord {
  [System.Windows.Forms.Clipboard]::SetText("NO SELECTION")
  $null = Await "the last word to be selected" { Press-SelectLastWord; Start-Sleep -Milliseconds 200; Chord @(,$LCTRL) 0x43 0x2E; Start-Sleep -Milliseconds 100; [System.Windows.Forms.Clipboard]::GetText() -cne "NO SELECTION" } 10000
}
# Waits until the app's field holds `$text` and takes keys: Ctrl+A, Ctrl+C returns it. Changes the clipboard. Input: the text.
function Wait-AppText($text) {
  $null = Await "the app's field to hold its text" { Chord @(,$LCTRL) 0x41 0x1E; Chord @(,$LCTRL) 0x43 0x2E; Start-Sleep -Milliseconds 100; [System.Windows.Forms.Clipboard]::GetText().Trim() -ceq $text } 15000
}
# The text of the focused element as UI Automation gives it (its text pattern, else its value), without touching the keyboard or the clipboard: typed text is still landing when the strip closes, so a copy would cut into it. Return: the text, empty when the element has neither.
function Focused-Text {
  try {
    $e = [Windows.Automation.AutomationElement]::FocusedElement
    $p = $null
    if ($e.TryGetCurrentPattern([Windows.Automation.TextPattern]::Pattern, [ref]$p)) { return $p.DocumentRange.GetText(-1).Trim() }
    if ($e.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern, [ref]$p)) { return $p.Current.Value.Trim() }
  } catch { }
  ""
}
# Reads the focused field's text until it equals `$want` or the time is up, and returns the last read. Inputs: the expected text, timeout in ms.
function Read-App($want, $ms) {
  $script:last = ""
  $null = WaitUntil { $script:last = Focused-Text; $script:last -ceq $want } $ms
  $script:last
}
# Opens a window with a classic Windows edit control (a multiline WinForms TextBox, run from a renamed copy of PowerShell because Cue treats powershell.exe as a terminal: the EDIT class every old app uses) holding `$text`, with the keyboard. Input: the text. Return: true.
function Open-Host($text) {
  $env:CUE_HOST_TEXT = $text
  $form = 'Add-Type -AssemblyName System.Windows.Forms; $f = New-Object Windows.Forms.Form; $f.Text = "Cue Edit Host"; $f.Width = 1100; $f.Height = 300; $t = New-Object Windows.Forms.TextBox; $t.Multiline = $true; $t.Dock = "Fill"; $t.Font = New-Object Drawing.Font("Segoe UI", 20); $t.Text = $env:CUE_HOST_TEXT; $t.Add_KeyDown({ if ($_.Control -and $_.KeyCode -eq "A") { $t.SelectAll(); $_.SuppressKeyPress = $true } }); $f.Controls.Add($t); $f.Add_Shown({ $f.Activate(); $t.Focus(); $t.Select($t.Text.Length, 0) }); [Windows.Forms.Application]::Run($f)'
  $exe = Join-Path $env:TEMP "cue-edit-host.exe"
  Copy-Item "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" $exe -Force
  Start-Process $exe -ArgumentList "-NoProfile", "-EncodedCommand", ([Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($form)))
  $null = Await "the edit host window" { (Win $null "Cue Edit Host") -ne [IntPtr]::Zero } 20000
  $null = Await "the edit host to take the keyboard" { $null = (New-Object -ComObject WScript.Shell).AppActivate("Cue Edit Host"); $h = Win $null "Cue Edit Host"; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 20000
  Wait-AppText $text
  $true
}
# Opens Windows 11 Notepad (WinUI, tabbed, with a menu bar the Alt key activates) with `$text` typed in its document, on its own new scratch file (a per-run name, deleted when its document closes), with the keyboard, and remembers the Notepad process ids it started (Start-OwnedNotepad starts one only when no Notepad was running and owns it only while it shows nothing but that file). Nothing is cleared: a document that already holds text is left alone and the case is skipped. Input: the text. Return: whether the app is ready.
function Open-Notepad($text) {
  [IO.File]::WriteAllText($notepadFile, "")
  $ids = Start-OwnedNotepad $notepadFile
  if ($null -eq $ids) { Remove-Item -LiteralPath $notepadFile; Write-Host "SKIPPED notepad: Notepad was already running, so it is not touched."; return $false }
  $script:ownedNotepad = @($ids)
  $null = Await "Notepad to take the keyboard" { $h = Win "Notepad" $null; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 20000
  $null = Await "Notepad's document to have the focus" { try { [Windows.Automation.AutomationElement]::FocusedElement.Current.ControlType.ProgrammaticName -eq "ControlType.Document" } catch { $false } } 20000
  if ((Focused-Text) -ne "") { Write-Host "SKIPPED notepad: its document already holds text, so it is not touched."; return $false }
  [System.Windows.Forms.Clipboard]::SetText($text); Chord @(,$LCTRL) 0x56 0x2F
  Wait-AppText $text
  $true
}
# Opens Edge on an editable page holding `$text`, with the keyboard. Input: the text. Return: true.
function Open-Edge($text) {
  [IO.File]::WriteAllText($page, "<title>Cue Text Access</title><body style='font:40px sans-serif'><div contenteditable id=e>$([Net.WebUtility]::HtmlEncode($text))</div><script>e.focus()</script>")
  $script:edge = Start-Process $Browser -ArgumentList "--user-data-dir=$profile", "--no-first-run", "--app=file:///$($page -replace '\\', '/')" -PassThru
  $null = Await "Edge to take the keyboard" { $h = Win $null "Cue Text Access"; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 20000
  Wait-AppText $text
  $true
}
# Closes the edit host, Edge and Notepad windows the cases opened: only this run's own Notepad document (then deletes its scratch file), the edit host copy and Edge's own profile.
function Close-Apps {
  Stop-OwnedNotepad $script:ownedNotepad $notepadFile
  Remove-Item -LiteralPath $notepadFile -ErrorAction SilentlyContinue
  Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like "*cue-text-access-profile*" -or $_.Name -eq "cue-edit-host.exe" } | ForEach-Object { Stop-Process $_.ProcessId -Force -ErrorAction SilentlyContinue }
  $null = Await "the opened apps to exit" { -not (Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like "*cue-text-access-profile*" -or $_.Name -eq "cue-edit-host.exe" }) } 20000
  $script:ownedNotepad = @()
}
# Presses the selection shortcut (Left Ctrl + Left Alt + Space) and keeps it down until the strip is visible, as a held shortcut is while Cue copies; throws when the strip never shows.
function Press-Selection {
  Hold @($LCTRL, $LALT) 0x20 0x39
  $shown = WaitUntil { Visible $null "Cue Transform Text" } 8000
  Release @($LCTRL, $LALT) 0x20 0x39
  if ($null -eq $shown) { throw "The transform strip was not shown 8 s after the selection shortcut." }
}
# Rewrites the selected text of the app through the strip and checks the app holds it. Inputs: label, script block that opens the app with `$sample`, window class or title lookup (`$class`, `$title`) of the app, seconds the strip is looked at before the chip key (a slow pick; the wait is the point of the case).
function Run-Transform($label, $open, $class, $title, $pickDelay = 0) {
  $want = "SEE HTTPS://A.COM/P?ID=1 NOW"
  if (-not (& $open $sample)) { Close-Apps; return }
  Chord @(,$LCTRL) 0x41 0x1E
  [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  $mark = Get-LogMark
  Press-Selection
  $null = Await "the strip to take the keyboard" { $h = Win $null "Cue Transform Text"; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 15000
  $null = Await "the strip page to paint" { (Read-LogSince $mark).Contains("strip: first paint reported") } 10000
  if ($pickDelay) { Start-Sleep $pickDelay }
  $script:stripLost = $false; $strip = Win $null "Cue Transform Text"; $before = New-Object Win32+RECT; $after = New-Object Win32+RECT
  $null = [Win32]::GetWindowRect($strip, [ref]$before)
  $script:keyed = [Diagnostics.Stopwatch]::StartNew(); Chord @() 0x31 0x02
  $null = Await "the result view to show (the strip changes size; key 1 is sent again while the page cannot take keys yet)" {
    $null = [Win32]::GetWindowRect($strip, [ref]$after)
    if ([Win32]::GetForegroundWindow() -ne $strip) { $script:stripLost = $true }
    if ($script:keyed.ElapsedMilliseconds -gt 500 -and $after.Bottom - $after.Top -eq $before.Bottom - $before.Top) { Chord @() 0x31 0x02; $script:keyed.Restart() }
    $after.Bottom - $after.Top -ne $before.Bottom - $before.Top } 10000
  if ($script:stripLost) { "$label NOTE: the strip was not the foreground window while keys were sent" }
  Chord @() 0x0D 0x1C
  $null = Await "the strip window to be gone after Enter (a press while it is still up only closes it)" { (Win $null "Cue Transform Text") -eq [IntPtr]::Zero } 10000
  $null = Await "$label to hold the keyboard again" { $h = Win $class $title; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 10000
  $kept = [System.Windows.Forms.Clipboard]::GetText() -eq "ORIGINAL CLIPBOARD"
  $text = Read-App $want 15000
  "$label log: " + (Read-LogSince $mark); "$label text: [$text]"
  Check "$label transform: the app holds the rewritten text" ($text -ceq $want)
  Check "$label transform: clipboard left as it was" $kept
  Close-Apps
}
# Looks up the last word of a sentence in the app and checks the Card opens. Inputs: label, script block that opens the app with a text.
function Run-Dictionary($label, $open) {
  if (-not (& $open "I drank some water")) { Close-Apps; return }
  Select-LastWord
  [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  $mark = Get-LogMark
  $e = [Windows.Automation.AutomationElement]::FocusedElement; "$label focus before the press: $($e.Current.ControlType.ProgrammaticName) $($e.Current.Name)"
  Chord @($LCTRL, $LALT) 0x44 0x20
  $shown = WaitUntil { Visible $null "Cue Dictionary" } 5000
  "$label log: " + (Read-LogSince $mark)
  Check "$label dictionary: the Card opens for the selected word" ($null -ne $shown -and (Read-LogSince $mark).Contains("dictionary: decided card"))
  Close-Apps
}

$clipboard = [System.Windows.Forms.Clipboard]::GetText()
Enter-UserData
try {
  New-Item -ItemType Directory -Force $dir | Out-Null
  Set-Content $settings '{"onboarded":true}'
  '{ "version": 1, "rules": [ { "name": "check", "trigger": "selection", "actions": ["clean_links", "upper_case"], "output": "insert" } ] }' | Set-Content $rules
  $mark = Get-LogMark
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  Run-Transform "edit   " ${function:Open-Host} $null "Cue Edit Host"
  Run-Transform "edge   " ${function:Open-Edge} $null "Cue Text Access"
  Run-Transform "notepad" ${function:Open-Notepad} "Notepad" $null
  Run-Transform "notepad slow pick" ${function:Open-Notepad} "Notepad" $null 4
  if (Test-Path $pack) {
    Run-Dictionary "edit   " ${function:Open-Host}
    Run-Dictionary "edge   " ${function:Open-Edge}
    Run-Dictionary "notepad" ${function:Open-Notepad}
  } else { "SKIPPED the dictionary cases: no dictionary Pack is installed ($pack not found)." }
} finally {
  Protected "close the apps" { Close-Apps }
  Protected "restore your settings, rules, notes and looked-up words" { Exit-UserData }
  Protected "restore the clipboard" { if ($clipboard) { [System.Windows.Forms.Clipboard]::SetText($clipboard) } else { [System.Windows.Forms.Clipboard]::Clear() } }
}
Complete-Checks
