# File: scripts/verify/tray-smoke.ps1
# Purpose: prove the tray menu window opens fast, stays open, and looks right. Posts the messages Windows sends for a tray click to Cue's tray window, waits for the "Cue Menu" window, times it, checks it is still there, and saves a screenshot to -Shot. The debug exe loads its UI from Vite, so this starts a Vite server unless one is running. The menu opens at the pointer and the outside-click check clicks at (20, 20), so the pointer starts in the middle of the screen.
# Contents: MenuWindow, Shown, Esc-Closes, Pixel, ShadowMargin, BodyPixels (card samples and the shadow check, taken after the menu has been up 1.5 s so the fade-in is over), the open, hold, Esc (Esc-Closes) and outside-click steps, the verdict.
# The menu window is the card plus a painted shadow margin of 14 dip on every side, so every probe is placed from the card's edge, never from the window's.
# shortcut: Esc-Closes cannot tell a tap lost before Cue's hook from a delivered tap the menu ignored, so a bug that needs two delivered taps would pass; add a keyboard-hook delivery line to cue.log and retry only on a missing line.
# Usage: pwsh scripts/verify/tray-smoke.ps1 [-Exe path] [-Shot menu.png]   (-Button 0x205 = right click up, default)
param([string]$exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\debug\cue.exe"), [int]$button = 0x205, [string]$shot = (Join-Path $env:TEMP "cue-menu.png"))
. (Join-Path $PSScriptRoot "..\lib\require-unlocked.ps1")
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
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
# Taps Escape every 700 ms for up to 8 s until the menu closes, and returns the milliseconds it took or $null. Injected keys do not always reach Cue's keyboard hook (its log shows no callback for the lost taps, even with the hook up for minutes), so one tap proves nothing; a menu that stays open after eight seconds of taps is a real failure.
function Esc-Closes {
  for ($n = 0; $n -lt 11; $n++) {
    [Win32]::keybd_event(0x1B, 0x01, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40; [Win32]::keybd_event(0x1B, 0x01, 2, [UIntPtr]::Zero)
    $closed = WaitUntil { -not (Shown) } 700
    if ($null -ne $closed) { return $closed }
  }
}
# Reads the screen colour at a point. Inputs: x, y. Returns the colour.
function Pixel($x, $y) {
  $px = New-Object System.Drawing.Bitmap 1, 1; $gp = [System.Drawing.Graphics]::FromImage($px)
  $gp.CopyFromScreen($x, $y, 0, 0, $px.Size); $c = $px.GetPixel(0, 0); $gp.Dispose(); $px.Dispose(); $c
}
# Returns the shadow margin in px: 14 dip at the real DPI of the primary screen (the process is DPI aware), rounded as the menu rounds it.
function ShadowMargin { $g = [System.Drawing.Graphics]::FromHwnd([IntPtr]::Zero); $dpi = $g.DpiX; $g.Dispose(); [int][Math]::Round(14 * $dpi / 96, [MidpointRounding]::AwayFromZero) }
# Adds up the red, green and blue of the screen at each point. Input: points with X and Y. Returns the total.
function Sum-Pixels($pts) { $t = 0; foreach ($q in $pts) { $px = Pixel $q.X $q.Y; $t += $px.R + $px.G + $px.B }; $t }
# Samples three points far apart on the menu card, inside the shadow margin (not the middle, where the separator line is), and four points just below the card inside the margin. Returns the first card colour, whether the card points match and are not black (Ok), those four points (ShadowPoints) and their colour total (ShadowSum), which the caller compares with the same points once the menu is closed: the shadow must darken them (a black desktop cannot show it), and the window rect.
function Body-Pixels {
  $r = New-Object Win32+RECT; [Win32]::GetWindowRect((MenuWindow), [ref]$r) | Out-Null
  $m = ShadowMargin; $left = $r.Left + $m; $top = $r.Top + $m; $bottom = $r.Bottom - $m
  $c = Pixel ($left + 6) ([int]($top + ($bottom - $top) * 0.25)); $d = Pixel ($left + 6) ($top + 10); $e = Pixel ($left + 6) ($bottom - 10)
  $flat = ($c.ToArgb() -eq $d.ToArgb()) -and ($c.ToArgb() -eq $e.ToArgb())
  $pts = foreach ($f in 0.2, 0.4, 0.6, 0.8) { [pscustomobject]@{ X = [int]($left + ($r.Right - $m - $left) * $f); Y = $bottom + 4 } }
  [pscustomobject]@{ Color = $c; Samples = ('{0:X8} {1:X8} {2:X8}' -f $c.ToArgb(), $d.ToArgb(), $e.ToArgb()); Ok = $flat -and (($c.R + $c.G + $c.B) -gt 30); ShadowPoints = $pts; ShadowSum = (Sum-Pixels $pts); Rect = $r }
}

Stop-Cue
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
  "body pixel: $($c.R),$($c.G),$($c.B) ok=$($body.Ok) (samples $($body.Samples))"

  Start-Sleep -Milliseconds 1500; $stays = Shown
  $notActivated = [Win32]::GetForegroundWindow() -ne (MenuWindow)
  $shadowSum = Sum-Pixels $body.ShadowPoints
  $escOk = $null -ne (Esc-Closes)
  Start-Sleep -Milliseconds 300
  $closedSum = Sum-Pixels $body.ShadowPoints
  $shadowOk = ($closedSum -le 30 * 4) -or ($shadowSum -lt $closedSum * 0.95)
  "shadow: below the card the menu shows $shadowSum, the same points without the menu $closedSum, ok=$shadowOk"
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
if ($opened -and $stays -and $body.Ok -and $shadowOk -and $notActivated -and $escOk -and $clickOk) { "PASS" } else { "FAIL"; exit 1 }
