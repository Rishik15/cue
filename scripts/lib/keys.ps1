# File: scripts/lib/keys.ps1
# Purpose: the one place the harnesses inject key events, so every key, chord and held shortcut is sent the same way and an extended key (arrow, Delete, End) gets its flag.
# Contents: $LCTRL, $LALT, $LSHIFT (vk, scan pairs), Send-Key, Tap, Chord, Hold, Release.
# Usage: . "$PSScriptRoot\..\lib\win32.ps1"; . "$PSScriptRoot\..\lib\keys.ps1" (load it after win32.ps1).
$LCTRL = @(0xA2, 0x1D); $LALT = @(0xA4, 0x38); $LSHIFT = @(0xA0, 0x2A)

# Sends one key event. Inputs: virtual key, scan code, whether it is the key up, whether it is an extended key.
function Send-Key($vk, $scan, [bool]$up, [bool]$extended) {
  [Win32]::keybd_event($vk, $scan, [uint32](2 * [int]$up + [int]$extended), [UIntPtr]::Zero)
}

# Presses and releases one key with no pause. Inputs: virtual key, scan code, whether it is an extended key.
function Tap($vk, $scan, [bool]$extended) { Send-Key $vk $scan $false $extended; Send-Key $vk $scan $true $extended }

# Sends a key chord: modifiers down, key down, 100 ms, key up, modifiers up in reverse. Inputs: modifier (vk, scan) pairs, key code, key scan code, whether the key is extended.
function Chord($mods, $key, $keyScan, [bool]$extended) {
  foreach ($m in $mods) { Send-Key $m[0] $m[1] $false $false }
  Send-Key $key $keyScan $false $extended; Start-Sleep -Milliseconds 100; Send-Key $key $keyScan $true $extended
  for ($i = $mods.Count - 1; $i -ge 0; $i--) { Send-Key $mods[$i][0] $mods[$i][1] $true $false }
}

# Presses a held shortcut: every modifier down, then the key down. Inputs: modifier (vk, scan) pairs, key code, key scan code.
function Hold($mods, $key, $keyScan) {
  foreach ($m in $mods) { Send-Key $m[0] $m[1] $false $false }
  Send-Key $key $keyScan $false $false
}

# Releases a held shortcut: the key up, then the modifiers up in reverse. Inputs: modifier (vk, scan) pairs, key code, key scan code.
function Release($mods, $key, $keyScan) {
  Send-Key $key $keyScan $true $false
  for ($i = $mods.Count - 1; $i -ge 0; $i--) { Send-Key $mods[$i][0] $mods[$i][1] $true $false }
}
