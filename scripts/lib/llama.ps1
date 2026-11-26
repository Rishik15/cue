# File: scripts/lib/llama.ps1
# Purpose: start, own and stop the lab scripts' llama-server processes so a lab run never ends a server it did not start.
# Contents: Assert-NoLlamaServer (refuse while one runs), Start-LlamaServer (the started process joins the caller's list before any wait), Stop-LlamaServers (the caller's finally: .NET calls only, because a cmdlet such as Stop-Process fails in a finally once Ctrl+C has stopped the pipeline, and Kill throws only when the process is already gone).
# Usage: dot-source, then: Assert-NoLlamaServer; $owned = [Collections.Generic.List[Diagnostics.Process]]::new(); try { $s = Start-LlamaServer $server $flags $port $owned 120000 } finally { Stop-LlamaServers $owned }
. (Join-Path $PSScriptRoot "wait.ps1")

# Throws when a process of that name runs, so the caller cannot end someone else's server. Input: the process name (the self-check passes its stand-in's).
function Assert-NoLlamaServer($name = "llama-server") {
  if (Get-Process $name -ErrorAction SilentlyContinue) { throw "A llama-server is already running; stop it first so this script cannot end someone else's server" }
}

# Starts a server and adds it to the caller's list before anything else runs, so the caller's finally stops it however the start ends (time out, exit, error, Ctrl+C, also after /health answered and before this returns). Inputs: the server exe, its arguments, the port it listens on, the caller's list of processes it owns and the readiness timeout in ms. Returns the process and the seconds until it answered /health, or $null when it did not start.
function Start-LlamaServer($server, $arguments, $port, $owned, $ms) {
  $started = Get-Date
  $p = Start-Process $server -ArgumentList $arguments -PassThru -WindowStyle Hidden
  $owned.Add($p)
  $ready = $null -ne (WaitUntil { $p.HasExited -or $(try { $null = Invoke-WebRequest "http://127.0.0.1:$port/health" -UseBasicParsing -TimeoutSec 2; $true } catch { $false }) } $ms) -and -not $p.HasExited
  if (-not $ready) { return $null }
  @{ Process = $p; Seconds = ((Get-Date) - $started).TotalSeconds }
}

# Stops every process in the caller's list and waits up to 10 s for each to exit; safe in a finally during Ctrl+C. Input: the list.
function Stop-LlamaServers($owned) { foreach ($o in $owned) { try { $o.Kill() } catch { }; $null = $o.WaitForExit(10000) } }
