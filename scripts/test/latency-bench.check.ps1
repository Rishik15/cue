# File: scripts/test/latency-bench.check.ps1
# Purpose: self-check of Start-Model and the per-model loop's cleanup in scripts/lab/text-latency-bench.ps1 that needs no model: a stand-in server (a compiled exe that only sleeps) must be stopped by the loop's own finally when the start is cancelled (Ctrl+C, done here by stopping its pipeline) during the readiness wait or after /health answered and before Start-Model returns, or when the wait throws, and handed back still running when it answers /health.
# Contents: Start-Case, Get-Standins, Gone, the cases (each started server must be in the caller's list, which the loop's finally stops), the verdict.
# Usage: & scripts/test/latency-bench.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")

$bench = Join-Path $PSScriptRoot "..\lab\text-latency-bench.ps1"
$ast = [Management.Automation.Language.Parser]::ParseFile($bench, [ref]$null, [ref]$null)
$startModel = $ast.Find({ param($a) $a -is [Management.Automation.Language.FunctionDefinitionAst] -and $a.Name -eq "Start-Model" }, $true).Extent.Text
$loopFinally = $ast.Find({ param($a) $a -is [Management.Automation.Language.TryStatementAst] -and $a.Finally -and $a.Finally.Extent.Text -like '*$owned*' }, $true).Finally.Extent.Text
$work = Join-Path $env:TEMP "cue-latency-check-$PID"
$standin = Join-Path $work "cue-standin-server.exe"

# Builds a PowerShell instance that runs the bench's own Start-Model with the stand-in as the server, after a setup script that may replace a function it uses; with -Cleanup the call sits in a try whose finally is the bench loop's own. Inputs: the setup text and the switch. Returns the instance, not yet started.
function Start-Case($setup, [switch]$Cleanup) {
  $call = if ($Cleanup) { "try { Start-Model 'none.gguf' 1 `$owned } finally $loopFinally" } else { "Start-Model 'none.gguf' 1 `$owned" }
  $ps = [powershell]::Create()
  $null = $ps.AddScript(". '$(Join-Path $PSScriptRoot "..\lib\wait.ps1")'; `$server = '$standin'; `$Ctx = 1; `$Ngl = 0; `$owned = [Collections.Generic.List[Diagnostics.Process]]::new()`n$startModel`n$setup`n$call")
  $ps
}

# Returns the stand-in server processes now running.
function Get-Standins { @(Get-Process cue-standin-server -ErrorAction SilentlyContinue) }

# Returns whether every stand-in is gone within 5 s: a killed process can still be listed for a moment, while a leaked one sleeps forever.
function Gone { $null -ne (WaitUntil { (Get-Standins).Count -eq 0 } 5000) }

New-Item -ItemType Directory $work | Out-Null
try {
  Add-Type -TypeDefinition "static class Standin { static void Main() { System.Threading.Thread.Sleep(-1); } }" -OutputAssembly $standin -OutputType ConsoleApplication

  $probed = Join-Path $work "probed"
  $ps = Start-Case "function Invoke-WebRequest { New-Item '$probed' -Force | Out-Null; throw 'not ready' }" -Cleanup
  $async = $ps.BeginInvoke()
  $null = Await "the readiness wait to probe /health" { Test-Path -LiteralPath $probed } 10000
  $ps.Stop(); $ps.Dispose()
  Check "Ctrl+C during the readiness wait stops the server" (Gone)

  $ps = Start-Case "function WaitUntil { throw 'wait failed' }" -Cleanup
  $thrown = try { $null = $ps.Invoke(); "" } catch { "$_" } finally { $ps.Dispose() }
  Check "an error during the readiness wait is rethrown" ($thrown -like "*wait failed*")
  Check "an error during the readiness wait stops the server" (Gone)

  $healthy = Join-Path $work "healthy"
  $ps = Start-Case "function Invoke-WebRequest {}; `$script:dates = 0; function Get-Date { if (`$script:dates++) { New-Item '$healthy' | Out-Null; Start-Sleep 60 }; [DateTime]::Now }" -Cleanup
  $async = $ps.BeginInvoke()
  $null = Await "the stand-in server to answer /health" { Test-Path -LiteralPath $healthy } 10000
  $ps.Stop(); $ps.Dispose()
  Check "Ctrl+C after /health answered and before Start-Model returns stops the server" (Gone)

  $ps = Start-Case "function Invoke-WebRequest {}"
  $ready = @($ps.Invoke()); $ps.Dispose()
  Check "a ready server is handed back still running" ($ready.Count -eq 1 -and -not $ready[0].Process.HasExited -and (Get-Standins).Count -eq 1)
} finally {
  Get-Standins | ForEach-Object { $_.Kill(); $null = $_.WaitForExit(10000) }
  Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
Complete-Checks
