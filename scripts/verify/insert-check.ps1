# File: scripts/verify/insert-check.ps1
# Purpose: verify text insertion into a real app (Notepad) with a debug build, whose `--insert-test <text> [paste]` mode inserts like a finished dictation: the typing path, the clipboard-paste path, and that the paste leaves your clipboard (text and image) as it was.
# Contents: Save-And-Read, Run-Case and the five checks.
# Usage: pwsh scripts/verify/insert-check.ps1 [-Exe path]   (build a debug exe first: cargo build --manifest-path src-tauri/Cargo.toml)
param([string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\debug\cue.exe"))
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\require-unlocked.ps1")
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$Text = "cue insert check"


# Saves Notepad's text with Ctrl+S, waits until the file holds the inserted text (or 10 s pass), and returns the file content. Input: the file path.
function Save-And-Read($file) {
  [Win32]::keybd_event(0x11, 0x1D, 0, [UIntPtr]::Zero); [Win32]::keybd_event(0x53, 0x1F, 0, [UIntPtr]::Zero)
  [Win32]::keybd_event(0x53, 0x1F, 2, [UIntPtr]::Zero); [Win32]::keybd_event(0x11, 0x1D, 2, [UIntPtr]::Zero)
  $null = WaitUntil { (Get-Content $file -Raw) -match $Text } 10000
  (Get-Content $file -Raw)
}

# Opens a fresh Notepad file, sets the clipboard, runs the insert test with the method and returns the file text, clipboard text and whether the clipboard still holds an image. Inputs: "type" or "paste", -Image to start from an image clipboard.
function Run-Case($method, [switch]$Image) {
  $file = Join-Path $env:TEMP "cue-insert-test.txt"; Set-Content $file ""
  Start-Process notepad -ArgumentList $file
  $null = Await "Notepad to show the test file" { @(Get-Process notepad -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -like "*cue-insert-test*" }).Count -gt 0 } 20000
  $null = Await "Notepad to take the focus" { (New-Object -ComObject WScript.Shell).AppActivate("cue-insert-test") } 5000
  if ($Image) {
    $bmp = New-Object System.Drawing.Bitmap 40, 40; [System.Windows.Forms.Clipboard]::SetImage($bmp)
  } else {
    [System.Windows.Forms.Clipboard]::SetText("ORIGINAL CLIPBOARD")
  }
  $err = Join-Path $env:TEMP "cue-insert.err"
  Start-Process $Exe -ArgumentList "--insert-test", "`"$Text`"", $method -Wait -NoNewWindow -RedirectStandardError $err
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
Complete-Checks
