# File: scripts/lab/text-security-check.ps1
# Purpose: prove the text model server is locked down the way Cue starts it: it needs the API key (given in the environment), refuses requests without it or with a wrong one, serves the key holder, and has the endpoint that lists recent prompts turned off. Uses the installed lfm2-1.2b model and the same flags as src-tauri/src/text/server.rs.
# Contents: Status, the server start with a random key and port, the five checks, the verdict.
# Usage: pwsh scripts/lab/text-security-check.ps1
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
$root = Join-Path $env:LOCALAPPDATA "dev.cue.app\models\text"
$server = Join-Path $root "server\llama-server.exe"
$model = Get-ChildItem (Join-Path $root "lfm2-1.2b") -Filter *.gguf | Select-Object -First 1
# Returns the HTTP status of a GET, or of a POST when a body is given; -1 when there is no response. Inputs: url, headers, optional JSON body.
function Status($url, $headers = @{}, $body = $null) {
  try { if ($body) { (Invoke-WebRequest $url -Method Post -Headers $headers -Body $body -ContentType "application/json" -UseBasicParsing -TimeoutSec 60).StatusCode } else { (Invoke-WebRequest $url -Headers $headers -UseBasicParsing -TimeoutSec 20).StatusCode } }
  catch { if ($_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { -1 } }
}
Get-Process llama-server -ErrorAction SilentlyContinue | Stop-Process -Force
$key = -join ((1..24) | ForEach-Object { "{0:x2}" -f (Get-Random -Maximum 256) })
$port = Get-Random -Minimum 20000 -Maximum 40000
$env:LLAMA_API_KEY = $key
$p = Start-Process $server -ArgumentList "--host 127.0.0.1 --port $port -c 4096 -np 1 -ngl 99 --no-webui --no-slots -t 2 -m `"$($model.FullName)`"" -PassThru -WindowStyle Hidden
Remove-Item Env:LLAMA_API_KEY
try {
  $null = Await "the text server to answer /health" { (Status "http://127.0.0.1:$port/health") -eq 200 } 60000
  $url = "http://127.0.0.1:$port"
  $body = '{"messages":[{"role":"user","content":"hi"}],"max_tokens":4}'
  Check "request without a key is refused" ((Status "$url/v1/chat/completions" @{} $body) -eq 401)
  Check "request with a wrong key is refused" ((Status "$url/v1/chat/completions" @{ Authorization = "Bearer wrong" } $body) -eq 401)
  Check "request with the right key is served" ((Status "$url/v1/chat/completions" @{ Authorization = "Bearer $key" } $body) -eq 200)
  $slots = Status "$url/slots" @{ Authorization = "Bearer $key" }
  Check "the recent-prompts endpoint is off (even with the key)" ($slots -ne 200)
  Check "the server listens on loopback only" (-not (Get-NetTCPConnection -LocalPort $port -State Listen | Where-Object { $_.LocalAddress -ne "127.0.0.1" }))
} finally { $p | Stop-Process -Force -ErrorAction SilentlyContinue }
Complete-Checks
