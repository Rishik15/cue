# Purpose: prove the window features do not leak. Opens and closes the snippets palette and the Grab Text selector 100 times each and compares private memory, handles and GDI/USER
# objects before and after; the app must still answer a press afterwards and no extra Cue process may be left. Your settings.json is set aside and restored (the checks press the default shortcuts).
# Needs a release build first (scripts/winsync release). Usage: pwsh scripts/soak.ps1 [-Exe path] [-Cycles 100]
param([string]$Exe = (Join-Path $PSScriptRoot "..\src-tauri\target\release\cue.exe"), [int]$Cycles = 100)
$ErrorActionPreference = "Stop"
Add-Type @'
using System; using System.Runtime.InteropServices;
public class K {
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flags);
}
'@
$settings = Join-Path $env:APPDATA "dev.cue.app\settings.json"; $backup = Join-Path $env:TEMP "cue-soak-settings.json"
if (Test-Path $settings) { Move-Item $settings $backup -Force }
function Chord($mods, $key) {
  foreach ($m in $mods) { [K]::keybd_event($m, 0, 0, [UIntPtr]::Zero) }
  [K]::keybd_event($key, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 60; [K]::keybd_event($key, 0, 2, [UIntPtr]::Zero)
  [array]::Reverse($mods); foreach ($m in $mods) { [K]::keybd_event($m, 0, 2, [UIntPtr]::Zero) }
}
$LCTRL = 0xA2; $LALT = 0xA4; $G = 0x47; $S = 0x53; $ESC = 0x1B
function Snapshot($p) { $p.Refresh(); [pscustomobject]@{ MB = [math]::Round($p.PrivateMemorySize64 / 1MB, 1); Handles = $p.HandleCount; Gdi = [K]::GetGuiResources($p.Handle, 0); User = [K]::GetGuiResources($p.Handle, 1) } }
function Wait-Preview($visible, $ms) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.ElapsedMilliseconds -lt $ms) {
    $h = [K]::FindWindow($null, "Cue Preview"); $now = ($h -ne [IntPtr]::Zero -and [K]::IsWindowVisible($h))
    if ($now -eq $visible) { return $true }
    Start-Sleep -Milliseconds 20
  }
  return $false
}
$checks = [ordered]@{}
function Check($name, $ok) { $script:checks[$name] = [bool]$ok; "{0,-58} {1}" -f $name, $(if ($ok) { "ok" } else { "FAILED" }) }
try {
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru; Start-Sleep 3
  1..5 | ForEach-Object { Chord @($LCTRL, $LALT) $S; Wait-Preview $true 5000 | Out-Null; Chord @() $ESC; Wait-Preview $false 3000 | Out-Null }   # warm-up: first use loads libraries
  Start-Sleep 2; $before = Snapshot $app; $opened = 0
  1..$Cycles | ForEach-Object {
    Chord @($LCTRL, $LALT) $S                                   # the snippets palette
    if (Wait-Preview $true 5000) { $opened++ }
    Chord @() $ESC; Wait-Preview $false 3000 | Out-Null
    Chord @($LCTRL, $LALT) $G; Start-Sleep -Milliseconds 400    # the grab selector: Esc cancels it
    Chord @() $ESC; Start-Sleep -Milliseconds 300
  }
  Start-Sleep 3; $after = Snapshot $app
  "before: $($before | ConvertTo-Json -Compress)"; "after:  $($after | ConvertTo-Json -Compress)"
  Check "snippets palette opened every time ($opened of $Cycles)" ($opened -eq $Cycles)
  Check "private memory grew by 3 MB or less" (($after.MB - $before.MB) -le 3)
  Check "handles grew by 20 or fewer" (($after.Handles - $before.Handles) -le 20)
  Check "GDI objects grew by 5 or fewer" (($after.Gdi - $before.Gdi) -le 5)
  Check "USER objects grew by 5 or fewer" (($after.User - $before.User) -le 5)
  Chord @($LCTRL, $LALT) $S; Check "still answers a press afterwards" (Wait-Preview $true 5000); Chord @() $ESC
  Start-Sleep 2
  Check "one Cue process only (no worker left behind)" (@(Get-Process cue -ErrorAction SilentlyContinue).Count -eq 1)
} finally {
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  if (Test-Path $backup) { Move-Item $backup $settings -Force }
}
if ($checks.Values -contains $false) { "FAIL"; exit 1 } else { "PASS" }
