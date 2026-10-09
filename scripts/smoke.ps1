# Purpose: prove the hotkey path end to end on Windows: injected Ctrl+Space must show the native overlay, release must hide it.
# Needs a release build first (pnpm tauri build --no-bundle). Usage: pwsh scripts/smoke.ps1
param([string]$exe = (Join-Path $PSScriptRoot "..\src-tauri\target\release\cue.exe"))
Add-Type @'
using System; using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string cls, string title);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
}
'@
Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
$p = Start-Process $exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-smoke.err")
Start-Sleep 3
$LCTRL = 0xA2; $SPACE = 0x20; $UP = 2
function Down { [W]::keybd_event($LCTRL, 0x1D, 0, [UIntPtr]::Zero); [W]::keybd_event($SPACE, 0x39, 0, [UIntPtr]::Zero) }
function Up { [W]::keybd_event($SPACE, 0x39, $UP, [UIntPtr]::Zero); [W]::keybd_event($LCTRL, 0x1D, $UP, [UIntPtr]::Zero) }
function Visible { Start-Sleep -Milliseconds 400; [W]::IsWindowVisible([W]::FindWindow("CueOverlay", [NullString]::Value)) }
Down; $shown = Visible
Up; $afterRelease = Visible
# Hold mode hides on release; Toggle mode needs a second press (works with whichever mode the user saved).
if ($afterRelease) { Down; Up; $afterRelease = Visible }
$p | Stop-Process -Force
"overlay shown on press: $shown, hidden after stop: $(-not $afterRelease)"
if ($shown -and -not $afterRelease) { "PASS" } else { "FAIL"; exit 1 }
