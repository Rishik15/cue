# File: scripts/test/cuelab.check.ps1
# Purpose: self-check of scripts/lib/cuelab.ps1 that starts no app: a lab result of the wrong length is refused by the count check Get-CueClean and Test-CueAnswers share, and a missing exe is named in the error. The app's answers themselves are covered by the Rust tests of `lab` and the Windows lab runs.
# Contents: Get-Thrown, the cases, the verdict.
# Usage: & scripts/test/cuelab.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\cuelab.ps1")

# Runs a block and returns the message it threw, or an empty string when it did not throw. Input: script block.
function Get-Thrown($block) { try { & $block; "" } catch { "$_" } }

$missing = Join-Path $env:TEMP "no-such-dir\cue.exe"
Check "a result list one short is refused" ((Get-Thrown { Assert-CueCount @(1, 2) 3 "judge" }) -eq "cue.exe --lab judge returned 2 results for 3 inputs")
Check "a single result for two inputs is refused" ((Get-Thrown { Assert-CueCount ([pscustomobject]@{ accepted = $true }) 2 "clean" }) -like "*returned 1 results for 2 inputs")
Check "a matching result list passes through as one array" ((Assert-CueCount @("a") 1 "clean").Count -eq 1)
Check "a missing exe is named in the error" ((Get-Thrown { Invoke-CueLab $missing @("dump-tasks") }) -eq "$missing not found. Build a release exe first (scripts\cue.ps1 release)")
Check "Get-CueTasks reports the missing exe too" ((Get-Thrown { Get-CueTasks $missing }) -like "*not found. Build a release exe first*")
Complete-Checks
