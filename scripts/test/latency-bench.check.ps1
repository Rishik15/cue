# File: scripts/test/latency-bench.check.ps1
# Purpose: self-check of Start-Model in scripts/lab/text-latency-bench.ps1 that needs no model: a stand-in server (a compiled exe that only sleeps) must be stopped when the readiness wait is cancelled (Ctrl+C, done here by stopping its pipeline) or throws, and handed back still running when it answers /health.
# Contents: Start-Case, Get-Standins, the cases, the verdict.
# Usage: & scripts/test/latency-bench.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")

$bench = Join-Path $PSScriptRoot "..\lab\text-latency-bench.ps1"
$startModel = [Management.Automation.Language.Parser]::ParseFile($bench, [ref]$null, [ref]$null).Find({ param($a) $a -is [Management.Automation.Language.FunctionDefinitionAst] -and $a.Name -eq "Start-Model" }, $true).Extent.Text
$work = Join-Path $env:TEMP "cue-latency-check-$PID"
$standin = Join-Path $work "cue-standin-server.exe"

# Builds a PowerShell instance that runs the bench's own Start-Model with the stand-in as the server, after a setup script that may replace a function the wait uses. Input: the setup text. Returns the instance, not yet started.
function Start-Case($setup) {
  $ps = [powershell]::Create()
  $null = $ps.AddScript(". '$(Join-Path $PSScriptRoot "..\lib\wait.ps1")'; `$server = '$standin'; `$Ctx = 1; `$Ngl = 0`n$startModel`n$setup`nStart-Model 'none.gguf' 1")
  $ps
}

# Returns the stand-in server processes now running.
function Get-Standins { @(Get-Process cue-standin-server -ErrorAction SilentlyContinue) }

New-Item -ItemType Directory $work | Out-Null
try {
  Add-Type -TypeDefinition "static class Standin { static void Main() { System.Threading.Thread.Sleep(-1); } }" -OutputAssembly $standin -OutputType ConsoleApplication

  $ps = Start-Case ""
  $async = $ps.BeginInvoke()
  $null = Await "the stand-in server to start" { (Get-Standins).Count -eq 1 } 10000
  Start-Sleep -Milliseconds 500
  $ps.Stop(); $ps.Dispose()
  Check "Ctrl+C during the readiness wait stops the server" ((Get-Standins).Count -eq 0)

  $ps = Start-Case "function WaitUntil { throw 'wait failed' }"
  $thrown = try { $null = $ps.Invoke(); "" } catch { "$_" } finally { $ps.Dispose() }
  Check "an error during the readiness wait is rethrown" ($thrown -like "*wait failed*")
  Check "an error during the readiness wait stops the server" ((Get-Standins).Count -eq 0)

  $ps = Start-Case "function Invoke-WebRequest {}"
  $ready = @($ps.Invoke()); $ps.Dispose()
  Check "a ready server is handed back still running" ($ready.Count -eq 1 -and -not $ready[0].Process.HasExited -and (Get-Standins).Count -eq 1)
} finally {
  Get-Standins | ForEach-Object { $_.Kill(); $null = $_.WaitForExit(10000) }
  Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
Complete-Checks
