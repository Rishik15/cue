# File: scripts/lib/require-unlocked.ps1
# Purpose: stop a check that injects keys when the Windows session is locked or showing another desktop, where injected keys reach no program and every key check would fail for a reason that is not a bug.
# Contents: the Guard.Desktop OpenInputDesktop import, and a guard line that prints UNSUPPORTED and exits 2 when the input desktop cannot be opened.
# Usage: . "$PSScriptRoot\..\lib\require-unlocked.ps1"
Add-Type -Namespace Guard -Name Desktop -MemberDefinition '/// Opens the desktop that receives input; fails (zero) when the session is locked or another desktop is active. Inputs: flags, inherit, access mask.
[DllImport("user32.dll")] public static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);'
if ([Guard.Desktop]::OpenInputDesktop(0, $false, 0x100) -eq [IntPtr]::Zero) { "UNSUPPORTED: no input desktop (the session is locked or has no desktop); this check did not run and is not a pass."; exit 2 }
