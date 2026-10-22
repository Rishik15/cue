# Purpose: run a command line and try again when it fails, because CI downloads (npm registry, corepack) sometimes time out for a minute and then work.
# Usage: pwsh scripts/retry.ps1 "<command line>"   (four tries, waiting 15, 30 and 45 seconds between them; the last failure is the result)
param([Parameter(Mandatory)][string]$Command)
for ($try = 1; $try -le 4; $try++) {
  Invoke-Expression $Command
  if ($LASTEXITCODE -eq 0) { exit 0 }
  "attempt $try failed with exit code $LASTEXITCODE"
  if ($try -lt 4) { Start-Sleep (15 * $try) }
}
exit $LASTEXITCODE
