# Purpose: prove the tray menu window opens fast, stays open, and looks right. Posts the messages Windows sends for a tray
# click to Cue's tray window, waits for the "Cue Menu" window, times it, checks it is still there, and saves a screenshot
# to -Shot. The debug exe loads its UI from Vite, so this starts a Vite server unless one is running.
# Usage: pwsh scripts/tray-smoke.ps1 [-Exe path] [-Shot menu.png]   (-Button 0x205 = right click up, default)
param([string]$exe = (Join-Path $PSScriptRoot "..\src-tauri\target\debug\cue.exe"), [int]$button = 0x205, [string]$shot = (Join-Path $env:TEMP "cue-menu.png"))
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public class M {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string cls, string title);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
'@
[M]::SetProcessDPIAware() | Out-Null   # real pixels, so coordinates and the screenshot match the physical screen
Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
$vite = $null
if ($exe -notmatch "release" -and -not (Test-NetConnection localhost -Port 1420 -InformationLevel Quiet -WarningAction SilentlyContinue)) {
  $vite = Start-Process cmd -ArgumentList "/c pnpm dev" -WorkingDirectory (Resolve-Path (Join-Path $PSScriptRoot "..")) -WindowStyle Hidden -PassThru
  for ($i = 0; $i -lt 40 -and -not (Test-NetConnection localhost -Port 1420 -InformationLevel Quiet -WarningAction SilentlyContinue); $i++) { Start-Sleep -Milliseconds 500 }
}
$p = Start-Process $exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-tray.err")
Start-Sleep 4
$tray = [M]::FindWindow("tray_icon_app", [NullString]::Value)
"tray window: $tray"
function MenuWindow { [M]::FindWindow([NullString]::Value, "Cue Menu") }
function Shown { $h = MenuWindow; ($h -ne [IntPtr]::Zero) -and [M]::IsWindowVisible($h) }
$WM_USER_TRAYICON = 6002
$sw = [Diagnostics.Stopwatch]::StartNew()
[M]::PostMessage($tray, $WM_USER_TRAYICON, [IntPtr]::Zero, [IntPtr]($button - 1)) | Out-Null   # button down
[M]::PostMessage($tray, $WM_USER_TRAYICON, [IntPtr]::Zero, [IntPtr]$button) | Out-Null          # button up
while (-not (Shown) -and $sw.ElapsedMilliseconds -lt 6000) { Start-Sleep -Milliseconds 20 }
$openMs = $sw.ElapsedMilliseconds; $opened = Shown
Start-Sleep -Milliseconds 800
$r = New-Object M+RECT; [M]::GetWindowRect((MenuWindow), [ref]$r) | Out-Null
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$w = 560; $h = 560; $bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen(($screen.Right - $w), ($screen.Bottom - $h), 0, 0, $bmp.Size); $bmp.Save($shot); $g.Dispose(); $bmp.Dispose()
# Pixel check without looking: the menu body must be the flat dark (or light) surface, not blank, black or the wallpaper.
$px = New-Object System.Drawing.Bitmap 1, 1
$gp = [System.Drawing.Graphics]::FromImage($px); $gp.CopyFromScreen($r.Left + 6, [int](($r.Top + $r.Bottom) / 2), 0, 0, $px.Size)
$c = $px.GetPixel(0, 0); $gp.Dispose(); $px.Dispose()
$bodyOk = ($c.R -eq $c.G) -and ($c.G -eq $c.B) -and ((($c.R -ge 40) -and ($c.R -le 48)) -or ($c.R -ge 245))
"body pixel: $($c.R),$($c.G),$($c.B) ok=$bodyOk"
Start-Sleep -Milliseconds 1500; $stays = Shown
# Pouchy behaviour: the menu never takes focus (so the tray overflow panel behind it stays open), Esc dismisses it through
# the keyboard hook, and a click outside dismisses it through the mouse hook.
$notActivated = [M]::GetForegroundWindow() -ne (MenuWindow)
[M]::keybd_event(0x1B, 0, 0, [UIntPtr]::Zero); [M]::keybd_event(0x1B, 0, 2, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 500; $escOk = -not (Shown)
[M]::PostMessage($tray, $WM_USER_TRAYICON, [IntPtr]::Zero, [IntPtr]($button - 1)) | Out-Null
[M]::PostMessage($tray, $WM_USER_TRAYICON, [IntPtr]::Zero, [IntPtr]$button) | Out-Null
$sw.Restart(); while (-not (Shown) -and $sw.ElapsedMilliseconds -lt 3000) { Start-Sleep -Milliseconds 20 }
[System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point(20, 20)
[M]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); [M]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 500; $clickOk = -not (Shown)
"not activated=$notActivated, Esc closes=$escOk, outside click closes=$clickOk"
"menu window: opened=$opened in $openMs ms, still open after 2.3 s=$stays, rect $($r.Left),$($r.Top) $($r.Right - $r.Left)x$($r.Bottom - $r.Top) px, process alive: $(-not $p.HasExited)"
$p | Stop-Process -Force
if ($vite) { taskkill /F /T /PID $vite.Id 2>&1 | Out-Null }
if ($opened -and $stays -and $bodyOk -and $notActivated -and $escOk -and $clickOk) { "PASS" } else { "FAIL"; exit 1 }
