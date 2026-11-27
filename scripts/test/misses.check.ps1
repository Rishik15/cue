# File: scripts/test/misses.check.ps1
# Purpose: self-check of scripts/lib/misses.ps1 on log text shaped like cue.log, so the repeat harness counts each kind of press under the right reason.
# Contents: the cases, the verdict.
# Usage: & scripts/test/misses.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\misses.ps1")
$press = "[1] selection: press`r`n"
$sent = "[1] selection: field Other in notepad (Notepad)`r`n[1] selection: copy keys sent: lifted [CTRL_LEFT | OPT_LEFT], 4 keys, to window 0x1 (Notepad) field None types [], clipboard 5`r`n"
$copied = $press + $sent + "[1] selection: copied in 40 ms`r`n[1] strip: visible at 300 ms`r`n"
$none = $press + $sent + "[3] selection: no clipboard update in 2000 ms, now window 0x1 (Notepad) field None types [], clipboard 5 (was 5)`r`n[3] selection: none in 2040 ms`r`n"

Check "an empty log never reached on_press" ((Get-PressOutcome "") -eq "never reached on_press")
Check "an empty log has not ended" (-not (Test-PressEnded ""))
Check "a press only has not ended" (-not (Test-PressEnded $press))
Check "a shown strip after a copy is copied" ((Get-PressOutcome $copied) -eq "copied" -and (Test-PressEnded $copied))
Check "a copy whose strip is not shown yet has not ended" (-not (Test-PressEnded ($press + $sent + "[1] selection: copied in 40 ms`r`n")))
Check "a timeout is none (no update)" ((Get-PressOutcome $none) -eq "none (no update)" -and (Test-PressEnded $none))
Check "a gate names its reason" ((Get-PressOutcome ($press + "[1] selection: press ignored: busy with dictation`r`n")) -eq "press ignored: busy with dictation")
Check "a moved focus is focus moved" ((Get-PressOutcome ($press + "[1] selection: the focus left the window before Ctrl+C, nothing was sent`r`n")) -eq "focus moved")
Check "an unproved copy is refused proof" ((Get-PressOutcome ($press + $sent + "[1] selection: copied in 40 ms`r`n[1] selection: the clipboard changed but not provably by the target, nothing used (Signs)`r`n")) -eq "refused proof")
Check "a trusted empty field is empty" ((Get-PressOutcome ($press + "[1] selection: field Empty in notepad (Notepad)`r`n[1] selection: Nothing selected, or Cue cannot see it`r`n")) -eq "empty (field said nothing selected)")
Check "a reported problem names it" ((Get-PressOutcome ($press + "[1] problem: Cue is busy with another clipboard operation. Try again.`r`n")) -eq "problem: Cue is busy with another clipboard operation. Try again.")
Complete-Checks
