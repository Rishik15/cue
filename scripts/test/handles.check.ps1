# File: scripts/test/handles.check.ps1
# Purpose: self-check of scripts/lib/handles.ps1 on real handles: this process gains one Process handle when it opens a child that then exits, and loses it when it lets go.
# Contents: the cases, the verdict.
# Usage: & scripts/test/handles.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\handles.ps1")
$before = [Handles]::ProcessCount([uint32]$PID)
$child = Start-Process cmd.exe -ArgumentList "/c", "exit 0" -WindowStyle Hidden -PassThru
$null = $child.Handle
$child.WaitForExit()
Check "a handle to an exited child counts as one Process handle" ([Handles]::ProcessCount([uint32]$PID) -eq $before + 1)
$child.Dispose()
Check "letting the handle go counts it no more" ([Handles]::ProcessCount([uint32]$PID) -eq $before)
Check "pid 3 (never given: ids are multiples of 4) holds none" ([Handles]::ProcessCount([uint32]3) -eq 0)
Complete-Checks
