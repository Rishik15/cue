# Purpose: end-to-end check of dictation into a real app without a microphone: a debug build's `--dictate-test <wav> <hold|live>` feeds a recording
# through the real speech path in real time while Notepad has focus; this script then saves the Notepad file, reads what landed in it and compares
# it with the expected words (word error rate). Needs the models the mode uses to be installed (live: a streaming model; hold: an offline one).
# Usage: pwsh scripts/dictate-check.ps1 -Kind live|hold [-Wav C:\dev\cue-long-test.wav] [-Expect C:\dev\cue-long-test.txt] [-Max 0.06]
param([string]$Kind = "live", [string]$Wav = "C:\dev\cue-long-test.wav", [string]$Expect = "C:\dev\cue-long-test.txt", [double]$Max = 0.06,
      [string]$Exe = (Join-Path $PSScriptRoot "..\src-tauri\target\debug\cue.exe"))
$ErrorActionPreference = "Stop"
Add-Type @'
using System; using System.Runtime.InteropServices;
public class K { [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra); }
'@
function Words($t) { ($t.ToLower() -replace "[^\p{L}\p{N}' ]", " ") -split "\s+" | Where-Object { $_ } }
function Edits($a, $b) {
  $row = 0..$b.Count
  for ($i = 1; $i -le $a.Count; $i++) {
    $prev = $row[0]; $row[0] = $i
    for ($j = 1; $j -le $b.Count; $j++) {
      $above = $row[$j]
      $row[$j] = [Math]::Min([Math]::Min($prev + [int]($a[$i - 1] -ne $b[$j - 1]), $row[$j - 1] + 1), $above + 1)
      $prev = $above
    }
  }
  $row[$b.Count]
}
$file = Join-Path $env:TEMP "cue-dictate-test.txt"; Set-Content $file ""
Start-Process notepad -ArgumentList $file; Start-Sleep 6
(New-Object -ComObject WScript.Shell).AppActivate("cue-dictate-test") | Out-Null; Start-Sleep 1
$err = Join-Path $env:TEMP "cue-dictate.err"
$secs = [int]((Get-Item $Wav).Length / 32000) + 60
$p = Start-Process $Exe -ArgumentList "--background", "--dictate-test", $Wav, $Kind -PassThru -NoNewWindow -RedirectStandardError $err
if (-not $p.WaitForExit($secs * 1000)) { $p.Kill(); "cue did not finish in $secs s" }
Start-Sleep 1
[K]::keybd_event(0x11, 0x1D, 0, [UIntPtr]::Zero); [K]::keybd_event(0x53, 0x1F, 0, [UIntPtr]::Zero)
[K]::keybd_event(0x53, 0x1F, 2, [UIntPtr]::Zero); [K]::keybd_event(0x11, 0x1D, 2, [UIntPtr]::Zero)
Start-Sleep 2
$typed = Get-Content $file -Raw
Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
$want = Words (Get-Content $Expect -Raw); $got = Words $typed
$edits = Edits $want $got; $rate = $edits / $want.Count
"stderr: " + ((Get-Content $err -ErrorAction SilentlyContinue) -join " | ")
"typed:  " + $typed.Trim()
"words {0} expected, {1} typed, {2} edits, error rate {3:P1}" -f $want.Count, $got.Count, $edits, $rate
if ($rate -le $Max) { "PASS" } else { "FAIL"; exit 1 }
