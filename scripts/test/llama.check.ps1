# File: scripts/test/llama.check.ps1
# Purpose: self-check of scripts/lib/llama.ps1 that needs no model: a stand-in server (a compiled exe that only sleeps) started by Start-LlamaServer must be stopped by a caller's finally that calls Stop-LlamaServers when the start is cancelled (Ctrl+C, done here by stopping its pipeline) during the readiness wait or after /health answered and before Start-LlamaServer returns, or when the wait throws, and handed back still running when it answers /health; Assert-NoLlamaServer refuses only while a process of that name runs; and every lab script that runs llama-server goes through the helper.
# Contents: Start-Case, Get-Standins, Gone, the cases, the lab-script scan, the verdict.
# Usage: & scripts/test/llama.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\llama.ps1")

$work = Join-Path $env:TEMP "cue-llama-check-$PID"
$standin = Join-Path $work "cue-standin-server.exe"

# Builds a PowerShell instance that runs Start-LlamaServer with the stand-in as the server, after a setup script that may replace a function it uses; with -Cleanup the call sits in a try whose finally calls Stop-LlamaServers, like the lab scripts. Inputs: the setup text and the switch. Returns the instance, not yet started.
function Start-Case($setup, [switch]$Cleanup) {
  $start = "Start-LlamaServer '$standin' 'none' 1 `$owned 120000"
  $call = if ($Cleanup) { "try { $start } finally { Stop-LlamaServers `$owned }" } else { $start }
  $ps = [powershell]::Create()
  $null = $ps.AddScript(". '$(Join-Path $PSScriptRoot "..\lib\llama.ps1")'; `$owned = [Collections.Generic.List[Diagnostics.Process]]::new()`n$setup`n$call")
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
  Check "Ctrl+C after /health answered and before the start returns stops the server" (Gone)

  $ps = Start-Case "function Invoke-WebRequest {}"
  $ready = @($ps.Invoke()); $ps.Dispose()
  Check "a ready server is handed back still running" ($ready.Count -eq 1 -and -not $ready[0].Process.HasExited -and (Get-Standins).Count -eq 1)
  $refused = try { Assert-NoLlamaServer cue-standin-server; "" } catch { "$_" }
  Check "Assert-NoLlamaServer refuses while one runs" ($refused -like "*already running*")
  Get-Standins | ForEach-Object { $_.Kill(); $null = $_.WaitForExit(10000) }
  $passed = try { Assert-NoLlamaServer cue-standin-server; $true } catch { $false }
  Check "Assert-NoLlamaServer passes when none runs" $passed

  $lab = @(Get-ChildItem (Join-Path $PSScriptRoot "..\lab") -Filter *.ps1 | Where-Object { (Get-Content -Raw $_.FullName) -like "*llama-server*" })
  $own = @($lab | Where-Object { $t = Get-Content -Raw $_.FullName; $t -like "*Assert-NoLlamaServer*" -and $t -like "*Stop-LlamaServers `$owned*" -and $t -notlike "*Start-Process `$server*" })
  Check "every lab script runs llama-server through scripts/lib/llama.ps1" ($lab.Count -eq 5 -and $own.Count -eq 5)
} finally {
  Get-Standins | ForEach-Object { $_.Kill(); $null = $_.WaitForExit(10000) }
  Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
Complete-Checks
