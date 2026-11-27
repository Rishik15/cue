# File: scripts/verify/dictionary-check.ps1
# Purpose: measure and prove the dictionary window end to end on Windows with the release build: a known word is put in a text box of a small helper window (a plain Windows text box, so the selection reads like any app's) and selected, the dictionary shortcut (Left Ctrl + Left Alt + D) is pressed and the Card must appear; then the same with nothing selected, which must open the Search window. For each it reports the time from the key press to the window being shown, the private MB of Cue and its web view processes while the window is open, and the private MB after it closed, and, which is checked, back at the idle figure Cue had before the window opened; it saves a screenshot of each in light and dark theme. Skips with a message when no dictionary Pack is installed. Your settings, notes and looked-up words are set aside and restored; the clipboard's text is put back, and a clipboard holding more than plain text is refused before anything changes.
# Contents: Close-Window, Shot, Measure-Case, Start-Host and the cases.
# Needs a release build first (scripts/cue.ps1 release). Usage: pwsh scripts/verify/dictionary-check.ps1 [-Exe path] [-Shots folder] [-SlackMB 4]
param(
  [string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"),
  [string]$Shots = (Join-Path $PSScriptRoot "..\..\.scratch\dictionary-window\shots"),
  [double]$SlackMB = 4
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
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$null = [Win32]::SetProcessDPIAware()
$word = "water"; $title = "Cue Dictionary"
$pack = Join-Path $env:LOCALAPPDATA "dev.cue.app\models\dictionary\dictionary-en\dictionary.db"
if (-not (Test-Path $pack)) { "SKIPPED: no dictionary Pack is installed ($pack not found); install the English Pack in Cue's settings and run this again."; exit 0 }
if (-not (Test-Path $Exe)) { throw "Build the release exe first (scripts\cue.ps1 release). Not found: $Exe" }
New-Item -ItemType Directory -Force $Shots | Out-Null
$Shots = (Resolve-Path $Shots).Path

# Presses Escape and waits until the dictionary window is gone; throws when it stays. Escape is the user's way to close both the Card and the Search window.
function Close-Window {
  Tap 0x1B 0x01
  $null = Await "the dictionary window to close" { (Win $app.Id $null $title) -eq [IntPtr]::Zero } 5000
}
# Saves a picture of the dictionary window's rectangle (with the margin its shadow uses) as a PNG. Inputs: file name.
function Shot($name) {
  $r = New-Object Win32+RECT; $null = [Win32]::GetWindowRect((Win $app.Id $null $title), [ref]$r)
  $bmp = New-Object Drawing.Bitmap ($r.Right - $r.Left), ($r.Bottom - $r.Top)
  [Drawing.Graphics]::FromImage($bmp).CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
  $bmp.Save((Join-Path $Shots $name), [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
}
# Presses the dictionary shortcut and measures one window: ms from the key going down to the window being visible, private MB once it has settled, and private MB after it closed against the idle figure, which is checked. Inputs: label, the idle snapshot, the shot's file name, the decision Cue's log must show (card or search).
function Measure-Case($label, $shot, $want) {
  $mark = Get-LogMark
  Hold @($LCTRL, $LALT) 0x44 0x20
  try { $ms = WaitUntil { Visible $app.Id $null $title } 5000 }
  finally { Release @($LCTRL, $LALT) 0x44 0x20 }
  Check "$label : window shown" ($null -ne $ms)
  if ($null -eq $ms) { return }
  Check "$label : Cue decided on a $want" (Test-LogLine $mark "dictionary_decided" $want)
  if ($want -eq "search") { $null = Await "the Search window to take the keyboard" { [Win32]::GetForegroundWindow() -eq (Win $app.Id $null $title) } 5000 }
  $open = Wait-Stable "private memory to stop moving while open" { (Snapshot $app.Id).PrivateMB } 0.5 30000
  Start-Sleep -Milliseconds 300
  Shot $shot
  Close-Window
  $after = Wait-Stable "private memory to stop moving after close" { (Snapshot $app.Id).PrivateMB } 0.5 30000
  $procs = (Snapshot $app.Id).Procs
  "{0}: press to shown {1} ms; private MB idle {2} -> open {3} -> closed {4}; processes {5} -> {6}" -f $label, $ms, $cold.PrivateMB, $open, $after, $cold.Procs, $procs
  Check "$label : back to idle (+$SlackMB MB, same processes)" ($after -le $cold.PrivateMB + $SlackMB -and $procs -le $cold.Procs)
}
# The helper window's code: a form with one text box that holds the word, selected and focused.
$hostCode = @"
Add-Type -AssemblyName System.Windows.Forms
`$f = New-Object Windows.Forms.Form; `$f.Text = 'Cue Dictionary Check Host'; `$f.TopMost = `$true; `$f.StartPosition = 'CenterScreen'; `$f.Width = 400; `$f.Height = 120
`$t = New-Object Windows.Forms.TextBox; `$t.Font = New-Object Drawing.Font('Segoe UI', 20); `$t.Text = '$word'; `$t.Dock = 'Top'; `$f.Controls.Add(`$t)
`$f.Add_Shown({ `$f.Activate(); `$t.Focus(); `$t.SelectAll() })
[Windows.Forms.Application]::Run(`$f)
"@
# Starts the helper window and waits until it holds the keyboard. Returns its process.
function Start-Host {
  $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($hostCode))
  $p = Start-Process powershell -ArgumentList "-NoProfile", "-STA", "-EncodedCommand", $encoded -PassThru
  $null = Await "the helper window to take the keyboard" { $h = Win $p.Id $null "Cue Dictionary Check Host"; $h -ne [IntPtr]::Zero -and [Win32]::GetForegroundWindow() -eq $h } 20000
  Start-Sleep -Milliseconds 500
  $p
}

Assert-TextClipboard
$clipboard = [System.Windows.Forms.Clipboard]::GetText()
Enter-UserData
try {
  New-Item -ItemType Directory -Force $dir | Out-Null
  foreach ($mode in "Light", "Dark") {
    Stop-Cue
    Set-Content $settings ('{"onboarded":true,"theme_mode":"' + $mode + '"}')
    $mark = Get-LogMark
    $app = Start-Process $Exe -ArgumentList "--background" -PassThru
    $null = Await "the tray window to exist" { (Win $app.Id "tray_icon_app" $null) -ne [IntPtr]::Zero } 10000
    Wait-Hotkeys $mark
    $null = Wait-Stable "idle private memory" { (Snapshot $app.Id).PrivateMB } 0.5 60000
    $cold = Snapshot $app.Id
    $helper = Start-Host
    try {
      [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
      Measure-Case "card   $mode" "card-$($mode.ToLower()).png" "card"
      Tap 0x27 0x4D $true
      Start-Sleep -Milliseconds 300
      Measure-Case "search $mode" "search-$($mode.ToLower()).png" "search"
    } finally { Stop-Process $helper -Force -ErrorAction SilentlyContinue }
  }
} finally {
  try { Exit-UserData } finally {
  if ($clipboard) { [System.Windows.Forms.Clipboard]::SetText($clipboard) } else { [System.Windows.Forms.Clipboard]::Clear() }
  }
}
Complete-Checks
