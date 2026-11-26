# File: scripts/test/keys.check.ps1
# Purpose: self-check of scripts/lib/keys.ps1 that sends no real key: Send-Key is replaced by a recorder, so it checks the event order of Tap, Chord, and Hold with Release in a finally, and that Chord and a held shortcut still send every key up when the held interval throws.
# Contents: Send-Key (the recorder), Start-Sleep (throws on demand), Invoke-Recorded, the cases, the verdict.
# Usage: & scripts/test/keys.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\keys.ps1")
$script:sent = New-Object Collections.Generic.List[string]
$script:sleepThrows = $false

# Records one key event as "<vk hex><d|u><e when extended>" instead of sending it. Inputs: virtual key, scan code, whether it is the key up, whether it is an extended key.
function Send-Key($vk, $scan, [bool]$up, [bool]$extended) { $script:sent.Add(("{0:X2}{1}{2}" -f [int]$vk, $(if ($up) { "u" } else { "d" }), $(if ($extended) { "e" } else { "" }))) }

# Stands in for the pause inside a held interval; throws when $sleepThrows is set, as a cancelled or failing harness would.
function Start-Sleep { if ($script:sleepThrows) { throw "interrupted" } }

# Clears the record, runs a block, swallows only the stand-in interruption and returns the recorded events as one space-separated string. Input: script block.
function Invoke-Recorded($block) {
  $script:sent.Clear()
  try { & $block } catch { if ("$_" -ne "interrupted") { throw } }
  $script:sent -join " "
}

Check "Tap sends down then up" ((Invoke-Recorded { Tap 0x58 0x2D }) -eq "58d 58u")
Check "Chord releases the key, then modifiers in reverse" ((Invoke-Recorded { Chord @($LCTRL, $LSHIFT) 0x25 0x4B $true }) -eq "A2d A0d 25de 25ue A0u A2u")
$script:sleepThrows = $true
Check "Chord sends every up when its pause throws" ((Invoke-Recorded { Chord @($LCTRL, $LALT) 0x44 0x20 $false }) -eq "A2d A4d 44d 44u A4u A2u")
Check "Hold with Release in finally releases on a throw" ((Invoke-Recorded { Hold @(,$LCTRL) 0x20 0x39; try { Start-Sleep } finally { Release @(,$LCTRL) 0x20 0x39 } }) -eq "A2d 20d 20u A2u")
Complete-Checks
