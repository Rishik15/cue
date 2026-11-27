# File: scripts/test/clipguard.check.ps1
# Purpose: self-check that every harness that writes the clipboard can give the user's clipboard back: it loads scripts/lib/clipformats.ps1, refuses a clipboard holding more than plain text (Assert-TextClipboard) on the line before it saves the text into $clipboard, and puts $clipboard back at the end. A new harness that writes the clipboard without this fails here.
# Contents: the scan of scripts/verify and scripts/lab, the per-harness checks, the verdict.
# Usage: & scripts/test/clipguard.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
$writers = @(Get-ChildItem (Join-Path $PSScriptRoot "..\verify"), (Join-Path $PSScriptRoot "..\lab") -Filter *.ps1 | Where-Object { (Get-Content -LiteralPath $_.FullName -Raw) -match 'Clipboard\]::Set' })
Check "the scan finds the seven clipboard-writing harnesses" ($writers.Count -ge 7)
foreach ($f in $writers) {
  $text = Get-Content -LiteralPath $f.FullName -Raw
  Check "$($f.Name): loads clipformats.ps1" ($text -match 'lib\\clipformats\.ps1')
  Check "$($f.Name): refuses non-text, then saves the text" ($text -match '(?m)^Assert-TextClipboard\r?\n\$clipboard = \[System\.Windows\.Forms\.Clipboard\]::GetText\(\)')
  Check "$($f.Name): puts the saved text back" ($text -match 'SetText\(\$clipboard\)')
}
Complete-Checks
