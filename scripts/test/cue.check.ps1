# File: scripts/test/cue.check.ps1
# Purpose: self-check of the mode table in scripts/cue.ps1: an unknown mode prints the usage and exits 2, help prints it and exits 0, the usage lists every mode once, and every harness a mode runs exists.
# Contents: Invoke-Cue, the cases, the verdict.
# Usage: & scripts/test/cue.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
$cue = Join-Path $PSScriptRoot "..\cue.ps1"

# Runs cue.ps1 in a child PowerShell with one mode. Input: the mode. Returns its exit code and printed text.
function Invoke-Cue($mode) {
  $out = & (Join-Path $PSHOME "powershell.exe") -NoProfile -ExecutionPolicy Bypass -File $cue $mode 2>&1 | Out-String
  [pscustomobject]@{ Code = $LASTEXITCODE; Text = $out }
}

$unknown = Invoke-Cue "nosuch"
Check "an unknown mode exits 2" ($unknown.Code -eq 2)
Check "an unknown mode prints the usage" ($unknown.Text -match '(?m)^\s+bench\s')
$help = Invoke-Cue "help"
Check "help exits 0" ($help.Code -eq 0)
$listed = @([regex]::Matches($help.Text, '(?m)^  (\S+)\s') | ForEach-Object { $_.Groups[1].Value })
Check "the usage lists each mode once" ($listed.Count -ge 30 -and @($listed | Sort-Object -Unique).Count -eq $listed.Count)
$harnesses = @([regex]::Matches((Get-Content -LiteralPath $cue -Raw), 'Invoke-Harness "([^"]+)"') | ForEach-Object { $_.Groups[1].Value })
Check "every harness a mode runs exists" ($harnesses.Count -ge 15 -and -not ($harnesses | Where-Object { -not (Test-Path (Join-Path $PSScriptRoot "..\$_")) }))
Complete-Checks
