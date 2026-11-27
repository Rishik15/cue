# File: scripts/test/edge.check.ps1
# Purpose: self-check of scripts/lib/edge.ps1 with a temp folder that has a space, so the profile and the page each stay one argument of Edge's command line.
# Contents: the cases, the verdict.
# Usage: & scripts/test/edge.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\edge.ps1")
$temp = "C:\Users\Jane Doe\AppData\Local\Temp\cue-run"
$line = Get-EdgeArguments "$temp\profile" "$temp\page.html" "textarea"
Check "a spaced profile is one quoted argument" ($line.StartsWith("`"--user-data-dir=$temp\profile`" "))
Check "a spaced page is one quoted escaped file URL" ($line.EndsWith(" `"--app=file:///C:/Users/Jane%20Doe/AppData/Local/Temp/cue-run/page.html#textarea`""))
Check "no fragment adds no hash" ((Get-EdgeArguments "p" "C:\x\page.html" "").EndsWith("page.html`""))
Complete-Checks
