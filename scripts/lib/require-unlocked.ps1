# Purpose: stop a check that injects keys when the Windows session is locked or showing another desktop, where injected keys reach no program and every key check would fail for a reason that is not a bug.
# Usage: . "$PSScriptRoot\..\lib\require-unlocked.ps1"
Add-Type -Namespace Guard -Name Desktop -MemberDefinition '[DllImport("user32.dll")] public static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);'
if ([Guard.Desktop]::OpenInputDesktop(0, $false, 0x100) -eq [IntPtr]::Zero) { "SKIP: the session is locked; unlock the screen and run this again."; exit 2 }
