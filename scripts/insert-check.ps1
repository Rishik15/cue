# Purpose: verify text insertion into a real app (Notepad) with a debug build, which types a one-line capture summary
# after each recording: the typing path, the clipboard-paste path, and that the paste leaves your clipboard (text and
# image) as it was. Your settings file is backed up and restored.
# Usage: pwsh scripts/insert-check.ps1 [-Exe path]   (build a debug exe first: cargo build --manifest-path src-tauri/Cargo.toml)
param([string]$Exe = (Join-Path $PSScriptRoot "..\src-tauri\target\debug\cue.exe"))
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public class K { [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra); }
'@
$settings = Join-Path $env:APPDATA "dev.cue.app\settings.json"
$backup = Join-Path $env:TEMP "cue-settings-backup.json"
$checks = [ordered]@{}
function Check($name, $ok) { $script:checks[$name] = [bool]$ok; "{0,-52} {1}" -f $name, $(if ($ok) { "ok" } else { "FAILED" }) }

function Set-InsertMethod($method) {
  $json = if (Test-Path $settings) { Get-Content $settings -Raw | ConvertFrom-Json } else { [pscustomobject]@{} }
  $json | Add-Member -NotePropertyName dictation_mode -NotePropertyValue "Hold" -Force
  $json | Add-Member -NotePropertyName insert_method -NotePropertyValue $method -Force
  $json | Add-Member -NotePropertyName restore_clipboard -NotePropertyValue $true -Force
  $json | ConvertTo-Json -Depth 5 | Set-Content $settings
}
function Dictate($seconds) {
  [K]::keybd_event(0xA2, 0x1D, 0, [UIntPtr]::Zero); [K]::keybd_event(0x20, 0x39, 0, [UIntPtr]::Zero)
  Start-Sleep -Seconds $seconds
  [K]::keybd_event(0x20, 0x39, 2, [UIntPtr]::Zero); [K]::keybd_event(0xA2, 0x1D, 2, [UIntPtr]::Zero)
}
# Notepad's text is read by saving the file (Ctrl+S) and reading it from disk, which needs no UI automation.
function Save-And-Read($file) {
  [K]::keybd_event(0x11, 0x1D, 0, [UIntPtr]::Zero); [K]::keybd_event(0x53, 0x1F, 0, [UIntPtr]::Zero)
  [K]::keybd_event(0x53, 0x1F, 2, [UIntPtr]::Zero); [K]::keybd_event(0x11, 0x1D, 2, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 1500
  (Get-Content $file -Raw)
}
function Run-Case($method, [switch]$Image) {
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  Set-InsertMethod $method
  $app = Start-Process $Exe -ArgumentList "--background" -PassThru -RedirectStandardError (Join-Path $env:TEMP "cue-insert.err")
  Start-Sleep 3
  # A fresh named file, so Notepad does not restore an old session and Ctrl+S saves without a dialog.
  $file = Join-Path $env:TEMP "cue-insert-test.txt"; Set-Content $file ""
  Start-Process notepad -ArgumentList $file
  Start-Sleep 4
  (New-Object -ComObject WScript.Shell).AppActivate("cue-insert-test") | Out-Null; Start-Sleep 1
  if ($Image) {
    $bmp = New-Object System.Drawing.Bitmap 40, 40; [System.Windows.Forms.Clipboard]::SetImage($bmp)
  } else {
    [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  }
  Dictate 1.5
  Start-Sleep 3
  $text = Save-And-Read $file
  $clip = [System.Windows.Forms.Clipboard]::GetText()
  $hasImage = [System.Windows.Forms.Clipboard]::ContainsImage()
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
  Stop-Process -Id $app.Id -Force
  Write-Host ("stderr: " + ((Get-Content (Join-Path $env:TEMP "cue-insert.err") -ErrorAction SilentlyContinue) -join " | "))
  [pscustomobject]@{ Text = $text; Clipboard = $clip; HasImage = $hasImage }
}

if (Test-Path $settings) { Copy-Item $settings $backup -Force }
try {
  $typed = Run-Case "Auto"
  Check "typing path inserted the capture line into Notepad" ($typed.Text -match "\[cue: [\d.]+s captured")
  Check "typing path left the clipboard alone" ($typed.Clipboard -eq "ORIGINAL CLIPBOARD")
  $pasted = Run-Case "Always Paste"
  Check "paste path inserted the capture line into Notepad" ($pasted.Text -match "\[cue: [\d.]+s captured")
  Check "paste path restored the previous clipboard text" ($pasted.Clipboard -eq "ORIGINAL CLIPBOARD")
  $image = Run-Case "Always Paste" -Image
  Check "paste path restored a copied image" $image.HasImage
} finally {
  Get-Process cue -ErrorAction SilentlyContinue | Stop-Process -Force
  if (Test-Path $backup) { Copy-Item $backup $settings -Force }
}
if ($checks.Values -contains $false) { "FAIL"; exit 1 } else { "PASS" }
