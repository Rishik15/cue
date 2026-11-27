# File: scripts/test/wait.check.ps1
# Purpose: self-check of Get-LogPattern in scripts/lib/wait.ps1: each log line the harnesses parse turns into a regex that matches the line as the app writes it (the same text the Rust test harness_log_lines_are_written_by_the_app checks against the source) and captures its numbers, and an unknown name throws.
# Contents: the cases, the verdict.
# Usage: & scripts/test/wait.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
$m = [regex]::Match("[1700000000] stage: keyboard hook ready at 12 ms on ThreadId(3) input", (Get-LogPattern "hook_ready"))
Check "hook_ready matches a stage line and captures its time" ($m.Success -and $m.Groups[1].Value -eq "12")
$m = [regex]::Match("[1700000000] expansion: start 3 ms, checks 4 ms, text 5 ms, typing 6 ms", (Get-LogPattern "expansion"))
Check "expansion captures its four marks" ($m.Success -and ((1..4 | ForEach-Object { $m.Groups[$_].Value }) -join ",") -eq "3,4,5,6")
Check "mic_opened matches its line" ("[1700000000] audio: microphone opened" -match (Get-LogPattern "mic_opened"))
Check "an unknown name throws" $(try { $null = Get-LogPattern "nosuch"; $false } catch { $true })
Complete-Checks
