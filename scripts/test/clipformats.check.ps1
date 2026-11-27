# File: scripts/test/clipformats.check.ps1
# Purpose: self-check of scripts/lib/clipformats.ps1, so a harness refuses an image, files or rich text on the clipboard and accepts plain text or nothing.
# Contents: the cases, the verdict.
# Usage: & scripts/test/clipformats.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\clipformats.ps1")
Check "an empty clipboard is text only" ((Get-NonTextFormats $null).Count -eq 0)
Check "plain text is text only" ((Get-NonTextFormats @("System.String", "UnicodeText", "Text", "Locale", "OEMText")).Count -eq 0)
Check "an image is named" (((Get-NonTextFormats @("Bitmap", "DeviceIndependentBitmap")) -join ",") -eq "Bitmap,DeviceIndependentBitmap")
Check "files are named" (((Get-NonTextFormats @("FileDrop", "Shell IDList Array")) -join ",") -eq "FileDrop,Shell IDList Array")
Check "rich text names only its extra formats" (((Get-NonTextFormats @("HTML Format", "Text", "UnicodeText")) -join ",") -eq "HTML Format")
Complete-Checks
