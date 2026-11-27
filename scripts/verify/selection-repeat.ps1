# File: scripts/verify/selection-repeat.ps1
# Purpose: count how often the Transform Text shortcut (Left Ctrl + Left Alt + Space) misses a known selection, and why, from cue.log: N presses into a per-run Notepad document and into a <textarea> and a contenteditable div of a local page in an Edge window this run owns. Each press selects the last word with keys, presses the shortcut, classifies the press from the log since it (scripts/lib/misses.ps1), closes the strip with Escape and waits until the strip window is gone. Prints one table of counts per target and reason; it measures, so misses do not fail it. Your settings, rules, notes and the clipboard's text are set aside and restored.
# Contents: Protected, Selected-Text, Select-Word, Focus-Target, Click-TitleBar, Close-Strip, Press-Once, Run-Target, Open-Notepad, Open-Edge, Close-Edge and the run.
# Needs a release build and Edge. Usage: pwsh scripts/verify/selection-repeat.ps1 [-Exe path] [-Presses 50]
param(
  [string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"),
  [string]$Browser = "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
  [int]$Presses = 50
)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\keys.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
. (Join-Path $PSScriptRoot "..\lib\require-unlocked.ps1")
. (Join-Path $PSScriptRoot "..\lib\userdata.ps1")
. (Join-Path $PSScriptRoot "..\lib\notepad.ps1")
. (Join-Path $PSScriptRoot "..\lib\misses.ps1")
Add-Type -AssemblyName System.Windows.Forms, UIAutomationClient, UIAutomationTypes
$rules = Join-Path $dir "rules.json"
$text = "alpha bravo charlie"
$word = "charlie"
$temp = Join-Path $env:TEMP ("cue-selection-repeat-" + [guid]::NewGuid())
$notepadFile = Join-Path $temp "cue-selection-repeat-notepad.txt"
$edgeProfile = Join-Path $temp "profile"
$script:ownedNotepad = @()
$script:edge = $null
$script:results = New-Object Collections.Generic.List[object]
$script:refocused = @{}
$null = [Win32]::SetProcessDPIAware()

# Runs one cleanup step and reports a failure instead of stopping, so one failed step cannot skip the restores after it. Inputs: what the step does, the step.
function Protected($what, $step) { try { & $step } catch { "WARNING: could not $what`: $_" } }
# The text UI Automation says is selected in the focused element, without touching the keyboard or the clipboard. Return: the text, empty when it cannot be read.
function Selected-Text {
  try {
    $p = $null
    if ([Windows.Automation.AutomationElement]::FocusedElement.TryGetCurrentPattern([Windows.Automation.TextPattern]::Pattern, [ref]$p)) { return (@($p.GetSelection()) | ForEach-Object { $_.GetText(-1) }) -join "" }
  } catch { }
  ""
}
# Selects the last word of the line with keys (End, then Ctrl+Shift+Left) until UI Automation shows it selected, so a lost key is the harness's miss, not Cue's. When that fails for 3 s (a shortcut Cue never saw can type into the field) the field's text is put back first (Escape, Ctrl+A, paste). A timeout throws.
function Select-Word {
  $select = { Tap 0x23 0x4F $true; Chord @($LCTRL, $LSHIFT) 0x25 0x4B $true; (Selected-Text).Trim() -ceq $word }
  if ($null -ne (WaitUntil $select 3000)) { return }
  "the field lost its text, putting it back"
  Tap 0x1B 0x01; Chord @(,$LCTRL) 0x41 0x1E; [System.Windows.Forms.Clipboard]::SetText($text); Chord @(,$LCTRL) 0x56 0x2F
  $null = Await "'$word' to be selected" $select 10000
}
# Waits until the target window is in front. When it is not back within 3 s (AppActivate is refused to a process that is not in front) the run notes which process holds the front and prints the log since the last press, counts it for the target and clicks the window's title bar, as a person would. Inputs: target label, the window title AppActivate matches, a script block returning the window handle. A timeout throws.
function Focus-Target($label, $title, $handle) {
  $front = { $h = & $handle; if ($h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -ne $h) { $null = (New-Object -ComObject WScript.Shell).AppActivate($title) }; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h }
  if ($null -ne (WaitUntil $front 3000)) { return }
  $fg = [Win32]::GetForegroundWindow()
  "$label the target was not in front after the strip closed; in front: " + (@(Get-Process | Where-Object { $_.MainWindowHandle -eq $fg } | ForEach-Object { "$($_.ProcessName) '$($_.MainWindowTitle)'" }) -join ", ")
  $script:refocused[$label] = 1 + [int]$script:refocused[$label]
  if ($script:lastMark) { "log since the last press:`n" + (Read-LogSince $script:lastMark) }
  Click-TitleBar (& $handle)
  $null = Await "$title to be in front" $front 10000
}
# Clicks 10 px below the top edge, in the middle, of a window: its title bar. shortcut: the position is normalized to the primary screen, so a window on another monitor is missed; use per-monitor coordinates if that shows up. Input: the window handle.
function Click-TitleBar($h) {
  $r = New-Object Win32+RECT; $null = [Win32]::GetWindowRect($h, [ref]$r)
  $screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  [Win32]::mouse_event(0x8001, [uint32](($r.Left + $r.Right) / 2 * 65535 / $screen.Width), [uint32](($r.Top + 10) * 65535 / $screen.Height), 0, [UIntPtr]::Zero)
  [Win32]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); [Win32]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
}
# Closes the strip with Escape, sent again each second it is still up (injected keys can miss while the strip is in front), and waits until its window is gone, so the next press is never a press that closes the strip. A timeout throws.
function Close-Strip {
  $sent = [Diagnostics.Stopwatch]::StartNew(); $first = $true
  $null = Await "the strip window to be gone" {
    $up = (Win $null "Cue Transform Text") -ne [IntPtr]::Zero
    if ($up -and ($first -or $sent.ElapsedMilliseconds -gt 1000) -and (Visible $null "Cue Transform Text")) { Tap 0x1B 0x01; $sent.Restart(); $first = $false }
    -not $up } 15000
}
# One press: select the word, press the shortcut, wait until the log says how the press ended (8 s; none at all is "never reached on_press"), then close the strip. Inputs: target label, window title, handle script block. Return: the outcome.
function Press-Once($label, $title, $handle) {
  Focus-Target $label $title $handle
  Select-Word
  # shortcut: a log rotation during the press makes Read-LogSince return the whole new log, which may hold earlier presses; mark by line count when that shows up.
  $mark = Get-LogMark; $script:lastMark = $mark
  Chord @($LCTRL, $LALT) 0x20 0x39
  $null = WaitUntil { Test-PressEnded (Read-LogSince $mark) } 8000
  Close-Strip
  $log = Read-LogSince $mark
  $outcome = Get-PressOutcome $log
  if ($outcome -ne "copied") { "$label miss ($outcome), log since the press:`n$log" }
  $script:results.Add([pscustomobject]@{ Target = $label; Outcome = $outcome })
}
# Presses `$Presses` times into one target. Inputs: target label, window title, handle script block.
function Run-Target($label, $title, $handle) { for ($i = 0; $i -lt $Presses; $i++) { Press-Once $label $title $handle } }
# Starts Notepad on this run's own file holding the text (only when no Notepad was running) and waits until its document has the keyboard. Return: whether Notepad is ready.
function Open-Notepad {
  [IO.File]::WriteAllText($notepadFile, $text)
  $ids = Start-OwnedNotepad $notepadFile
  if ($null -eq $ids) { Write-Host "SKIPPED notepad: Notepad was already running, so it is not touched."; return $false }
  $script:ownedNotepad = @($ids)
  $null = Await "Notepad's document to have the focus" { try { [Windows.Automation.AutomationElement]::FocusedElement.Current.ControlType.ProgrammaticName -eq "ControlType.Document" } catch { $false } } 20000
  $true
}
# Opens the local page in a new Edge app window with this run's own profile, with the keyboard in the element `$id` (textarea or editable). Input: the element id.
function Open-Edge($id) {
  $page = Join-Path $temp "page.html"
  [IO.File]::WriteAllText($page, "<title>Cue Selection Repeat</title><body style='font:32px sans-serif'><textarea id=textarea cols=30 rows=2>$text</textarea><div contenteditable id=editable>$text</div><script>document.getElementById(location.hash.slice(1)).focus()</script>")
  $script:edge = Start-Process $Browser -ArgumentList "--user-data-dir=$edgeProfile", "--no-first-run", "--new-window", "--app=file:///$($page -replace '\\', '/')#$id" -PassThru
  $null = Await "Edge to take the keyboard" { $h = Win $null "Cue Selection Repeat"; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 30000
}
# Ends the Edge processes that run this run's own profile, and only those, and waits until they are gone.
function Close-Edge {
  $own = { Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like "*$edgeProfile*" } }
  & $own | ForEach-Object { Stop-Process $_.ProcessId -Force -ErrorAction SilentlyContinue }
  $null = Await "this run's Edge to exit" { -not (& $own) } 20000
  $script:edge = $null
}

$clipboard = [System.Windows.Forms.Clipboard]::GetText()
Enter-UserData
try {
  New-Item -ItemType Directory -Force $dir, $temp | Out-Null
  Set-Content $settings '{"onboarded":true}'
  '{ "version": 1, "rules": [ { "name": "check", "trigger": "selection", "actions": ["upper_case"], "output": "insert" } ] }' | Set-Content $rules
  $mark = Get-LogMark
  $null = Start-Process $Exe -ArgumentList "--background" -PassThru
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 10000
  Wait-Hotkeys $mark
  if (Open-Notepad) {
    Run-Target "notepad" ([IO.Path]::GetFileName($notepadFile)) { Win "Notepad" $null }
    Stop-OwnedNotepad $script:ownedNotepad $notepadFile; $script:ownedNotepad = @()
  }
  foreach ($id in "textarea", "editable") {
    Open-Edge $id
    Run-Target "edge $id" "Cue Selection Repeat" { Win $null "Cue Selection Repeat" }
    Close-Edge
  }
  ""
  $script:results | Group-Object Target, Outcome | ForEach-Object { [pscustomobject]@{ Target = $_.Group[0].Target; Outcome = $_.Group[0].Outcome; Count = $_.Count } } | Sort-Object Target, @{ Expression = "Count"; Descending = $true } | Format-Table -AutoSize | Out-String
  foreach ($k in $script:refocused.Keys) { "$k`: the target was not in front after the strip closed $($script:refocused[$k]) times (the title bar was clicked)" }
} finally {
  Protected "close Notepad" { Stop-OwnedNotepad $script:ownedNotepad $notepadFile }
  Protected "close Edge" { if ($script:edge) { Close-Edge } }
  Protected "delete the run's files" { Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue }
  Protected "restore your settings, rules, notes and looked-up words" { Exit-UserData }
  Protected "restore the clipboard" { if ($clipboard) { [System.Windows.Forms.Clipboard]::SetText($clipboard) } else { [System.Windows.Forms.Clipboard]::Clear() } }
}
