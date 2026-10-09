# Purpose: show which Cue thread burns CPU while idle (per-thread CPU delta over 60 s). Needs a release build first (pnpm tauri build --no-bundle).
Add-Type @'
using System; using System.Runtime.InteropServices;
public class T {
  [DllImport("kernel32.dll")] public static extern IntPtr OpenThread(uint access, bool inherit, uint id);
  [DllImport("kernel32.dll")] public static extern int GetThreadDescription(IntPtr h, out IntPtr desc);
  [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
  public static string Name(uint id) { IntPtr h = OpenThread(0x0800, false, id); if (h == IntPtr.Zero) return "?"; IntPtr d; GetThreadDescription(h, out d); string s = Marshal.PtrToStringUni(d); CloseHandle(h); return string.IsNullOrEmpty(s) ? "(unnamed)" : s; }
}
'@
param([string]$exe = (Join-Path $PSScriptRoot "..\src-tauri\target\release\cue.exe"))
Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep 2
$p = Start-Process $exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-threads.err")
Start-Sleep 15
function Snap { (Get-Process -Id $p.Id).Threads | ForEach-Object { [pscustomobject]@{ Id = $_.Id; Cpu = $_.TotalProcessorTime.TotalMilliseconds; State = $_.ThreadState; Wait = $_.WaitReason } } }
$a = Snap; Start-Sleep 60; $b = Snap
foreach ($t in $b) { $o = $a | Where-Object Id -eq $t.Id; "{0,6} {5,-10} cpu={1,8:N1} ms  delta={2,6:N1} ms  {3} {4}" -f $t.Id, $t.Cpu, ($t.Cpu - $o.Cpu), $t.State, $t.Wait, [T]::Name($t.Id) }
$p | Stop-Process -Force
