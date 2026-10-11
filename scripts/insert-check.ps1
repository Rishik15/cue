# Purpose: verify text insertion into a real app (Notepad) with a debug build, whose `--insert-test <text> [paste]` mode inserts
# like a finished dictation: the typing path, the clipboard-paste path, and that the paste leaves your clipboard (text and image) as it was.
# Usage: pwsh scripts/insert-check.ps1 [-Exe path]   (build a debug exe first: cargo build --manifest-path src-tauri/Cargo.toml)
param([string]$Exe = (Join-Path $PSScriptRoot "..\src-tauri\target\debug\cue.exe"))
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public class K { [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra); }
'@
$checks = [ordered]@{}
function Check($name, $ok) { $script:checks[$name] = [bool]$ok; "{0,-52} {1}" -f $name, $(if ($ok) { "ok" } else { "FAILED" }) }

# Notepad's text is read by saving the file (Ctrl+S) and reading it from disk, which needs no UI automation.
function Save-And-Read($file) {
  [K]::keybd_event(0x11, 0x1D, 0, [UIntPtr]::Zero); [K]::keybd_event(0x53, 0x1F, 0, [UIntPtr]::Zero)
  [K]::keybd_event(0x53, 0x1F, 2, [UIntPtr]::Zero); [K]::keybd_event(0x11, 0x1D, 2, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 1500
  (Get-Content $file -Raw)
}
$Text = "cue insert check"
function Run-Case($method, [switch]$Image) {
  # A fresh named file, so Notepad does not restore an old session and Ctrl+S saves without a dialog.
  $file = Join-Path $env:TEMP "cue-insert-test.txt"; Set-Content $file ""
  Start-Process notepad -ArgumentList $file
  Start-Sleep 6
  (New-Object -ComObject WScript.Shell).AppActivate("cue-insert-test") | Out-Null; Start-Sleep 1
  if ($Image) {
    $bmp = New-Object System.Drawing.Bitmap 40, 40; [System.Windows.Forms.Clipboard]::SetImage($bmp)
  } else {
    [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  }
  $err = Join-Path $env:TEMP "cue-insert.err"
  Start-Process $Exe -ArgumentList "--insert-test", "`"$Text`"", $method -Wait -NoNewWindow -RedirectStandardError $err
  Start-Sleep 1
  $text = Save-And-Read $file
  $clip = [System.Windows.Forms.Clipboard]::GetText()
  $hasImage = [System.Windows.Forms.Clipboard]::ContainsImage()
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
  Write-Host ("read: [" + $text + "] stderr: " + ((Get-Content $err -ErrorAction SilentlyContinue) -join " | "))
  [pscustomobject]@{ Text = $text; Clipboard = $clip; HasImage = $hasImage }
}

try {
  $typed = Run-Case "type"
  Check "typing path inserted the text into Notepad" ($typed.Text -match $Text)
  Check "typing path left the clipboard alone" ($typed.Clipboard -eq "ORIGINAL CLIPBOARD")
  $pasted = Run-Case "paste"
  Check "paste path inserted the text into Notepad" ($pasted.Text -match $Text)
  Check "paste path restored the previous clipboard text" ($pasted.Clipboard -eq "ORIGINAL CLIPBOARD")
  $image = Run-Case "paste" -Image
  Check "paste path restored a copied image" $image.HasImage
} finally {
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
}
if ($checks.Values -contains $false) { "FAIL"; exit 1 } else { "PASS" }
