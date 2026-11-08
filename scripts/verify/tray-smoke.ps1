# File: scripts/verify/tray-smoke.ps1
# Purpose: prove the tray menu window opens fast, stays open, and looks right. Posts the messages Windows sends for a tray click to Cue's tray window, waits for the "Cue Menu" window, times it, checks it is still there, and saves a screenshot to -Shot. The debug exe loads its UI from Vite, so this starts a Vite server unless one is running. The menu opens at the pointer and the outside-click check clicks at (20, 20), so the pointer starts in the middle of the screen.
# Contents: MenuWindow, Shown, Pixel, BodyPixels, the open, hold, Esc and outside-click steps, the verdict.
# Usage: pwsh scripts/verify/tray-smoke.ps1 [-Exe path] [-Shot menu.png]   (-Button 0x205 = right click up, default)
param([string]$exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\debug\cue.exe"), [int]$button = 0x205, [string]$shot = (Join-Path $env:TEMP "cue-menu.png"))
. (Join-Path $PSScriptRoot "..\lib\require-unlocked.ps1")
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$WM_USER_TRAYICON = 6002
[System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point(700, 500)
[Win32]::SetProcessDPIAware() | Out-Null

# Returns the handle of the tray menu window, or zero.
function MenuWindow { [Win32]::FindWindow([NullString]::Value, "Cue Menu") }
# Returns whether the tray menu window exists and is visible.
function Shown { $h = MenuWindow; ($h -ne [IntPtr]::Zero) -and [Win32]::IsWindowVisible($h) }
# Returns whether the Vite dev server answers on port 1420.
function ViteUp { Test-NetConnection localhost -Port 1420 -InformationLevel Quiet -WarningAction SilentlyContinue }
# Posts the messages Windows sends for a tray click (button down, then up) to the tray window. Input: tray window handle.
function Click-Tray($tray) {
  [Win32]::PostMessage($tray, $WM_USER_TRAYICON, [IntPtr]::Zero, [IntPtr]($button - 1)) | Out-Null
  [Win32]::PostMessage($tray, $WM_USER_TRAYICON, [IntPtr]::Zero, [IntPtr]$button) | Out-Null
}
# Reads the screen colour at a point. Inputs: x, y. Returns the colour.
function Pixel($x, $y) {
  $px = New-Object System.Drawing.Bitmap 1, 1; $gp = [System.Drawing.Graphics]::FromImage($px)
  $gp.CopyFromScreen($x, $y, 0, 0, $px.Size); $c = $px.GetPixel(0, 0); $gp.Dispose(); $px.Dispose(); $c
}
# Samples three points far apart on the menu body (not the middle, where the separator line is). Returns the first colour and whether all three match and are not black.
function Body-Pixels {
  $r = New-Object Win32+RECT; [Win32]::GetWindowRect((MenuWindow), [ref]$r) | Out-Null
  $c = Pixel ($r.Left + 6) ([int]($r.Top + ($r.Bottom - $r.Top) * 0.25)); $d = Pixel ($r.Left + 6) ($r.Top + 10); $e = Pixel ($r.Left + 6) ($r.Bottom - 10)
  $flat = ($c.ToArgb() -eq $d.ToArgb()) -and ($c.ToArgb() -eq $e.ToArgb())
  [pscustomobject]@{ Color = $c; Ok = $flat -and (($c.R + $c.G + $c.B) -gt 30); Rect = $r }
}

Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
$null = Wait-Process -Name cue -Timeout 10 -ErrorAction SilentlyContinue
$vite = $null; $p = $null
try {
  if ($exe -notmatch "release" -and -not (ViteUp)) {
    $vite = Start-Process cmd -ArgumentList "/c pnpm dev" -WorkingDirectory (Resolve-Path (Join-Path $PSScriptRoot "..\..")) -WindowStyle Hidden -PassThru
    $null = Await "the Vite server to listen on port 1420" { ViteUp } 20000
  }
  $mark = Get-LogMark
  $p = Start-Process $exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-tray.err")
  $null = Await "the tray window to exist" { [Win32]::FindWindow("tray_icon_app", [NullString]::Value) -ne [IntPtr]::Zero } 15000
  Wait-Hotkeys $mark
  $tray = [Win32]::FindWindow("tray_icon_app", [NullString]::Value)
  "tray window: $tray"

  Click-Tray $tray
  $openMs = WaitUntil { Shown } 6000
  $opened = $null -ne $openMs

  $null = WaitUntil { (Body-Pixels).Ok } 5000
  $body = Body-Pixels; $r = $body.Rect; $c = $body.Color
  $screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $w = 560; $h = 560; $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen(($screen.Right - $w), ($screen.Bottom - $h), 0, 0, $bmp.Size); $bmp.Save($shot); $g.Dispose(); $bmp.Dispose()
  "body pixel: $($c.R),$($c.G),$($c.B) ok=$($body.Ok)"

  Start-Sleep -Milliseconds 1500; $stays = Shown
  $notActivated = [Win32]::GetForegroundWindow() -ne (MenuWindow)
  [Win32]::keybd_event(0x1B, 0, 0, [UIntPtr]::Zero); [Win32]::keybd_event(0x1B, 0, 2, [UIntPtr]::Zero)
  $escOk = $null -ne (WaitUntil { -not (Shown) } 2000)
  Click-Tray $tray
  $null = WaitUntil { Shown } 3000
  [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point(20, 20)
  [Win32]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); [Win32]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
  $clickOk = $null -ne (WaitUntil { -not (Shown) } 2000)
  "not activated=$notActivated, Esc closes=$escOk, outside click closes=$clickOk"
  "menu window: opened=$opened in $openMs ms, still open after the hold=$stays, rect $($r.Left),$($r.Top) $($r.Right - $r.Left)x$($r.Bottom - $r.Top) px, process alive: $(-not $p.HasExited)"
} finally {
  if ($p) { $p | Stop-Process -Force -ErrorAction SilentlyContinue }
  if ($vite) { taskkill /F /T /PID $vite.Id 2>&1 | Out-Null }
}
if ($opened -and $stays -and $body.Ok -and $notActivated -and $escOk -and $clickOk) { "PASS" } else { "FAIL"; exit 1 }
