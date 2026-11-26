# File: scripts/test/proc.check.ps1
# Purpose: self-check of Percentile in scripts/lib/proc.ps1, which every bench number goes through: the nearest-rank median and 95th percentile of 1..10, and $null for an empty list.
# Contents: the cases, the verdict.
# Usage: & scripts/test/proc.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
Check "the 50th percentile of 1..10 is 5" ((Percentile (10..1) 50) -eq 5)
Check "the 95th percentile of 1..10 is 10" ((Percentile (1..10) 95) -eq 10)
Check "the percentile of an empty list is null" ($null -eq (Percentile @() 50))
Complete-Checks
