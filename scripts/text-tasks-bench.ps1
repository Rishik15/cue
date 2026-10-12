# Purpose: rate the rewrite tasks (tidy_speech, bullet_list, summarize, simplify) that have no reference answer. Reads each task's prompt and examples from
# src-tauri/src/actions/model.rs (so it cannot drift), runs bench/task_cases.tsv through llama-server like Cue does, and checks each answer by rule:
# protected tokens kept (ignoring case, spaces, commas and $, so "3 PM" counts as 3pm), bullets start with "- ", summary and tidy output not longer than the input, no filler left, order-like text not obeyed (not more than 3x the input).
# Prints a pass rate per task and model, and every answer with -Show so a person can read them. Usage: pwsh scripts/text-tasks-bench.ps1 [-Only id] [-Passes 2] [-Show]
param([string[]]$Only = @(), [int]$Passes = 2, [switch]$Show)
$ErrorActionPreference = "Stop"
$root = Join-Path $env:LOCALAPPDATA "dev.cue.app\models\text"
$server = Join-Path $root "server\llama-server.exe"
$src = Get-Content (Join-Path $PSScriptRoot "..\src-tauri\src\actions\model.rs") -Raw
$guard = [regex]::Match($src, 'macro_rules! guard \{.*?"(.*?)"\s*\}', "Singleline").Groups[1].Value
function Unescape($s) { $s.Replace('\"', '"').Replace('\n', "`n") }
function Literals($block) { [regex]::Matches($block, '"((?:[^"\\]|\\.)*)"') | ForEach-Object { Unescape $_.Groups[1].Value } }
$names = @{ tidy_speech = "TIDY"; bullet_list = "BULLETS"; summarize = "SUMMARY"; simplify = "SIMPLER" }
$tasks = @{}
foreach ($id in $names.Keys) {
  $block = [regex]::Match($src, "static $($names[$id]): Task = Task \{(.*?)\n\};", "Singleline").Groups[1].Value
  $sys = [regex]::Match($block, 'system: concat!\((.*?)guard!\(\)', "Singleline").Groups[1].Value
  $shots = [regex]::Match($block, 'shots: &\[(.*?)\n    \],', "Singleline").Groups[1].Value
  $lits = @(Literals $shots)
  $tasks[$id] = @{ System = ((Literals $sys) -join "") + $guard; Shots = $lits }
}
$cases = Get-Content (Join-Path $PSScriptRoot "..\bench\task_cases.tsv") | ForEach-Object { $p = $_ -split "`t"; [pscustomobject]@{ Task = $p[0]; Input = $p[1]; Tokens = @(($p[2] + "") -split "\|" | Where-Object { $_ }) } }
function Squash($s) { ($s.ToLower() -replace "[\s,$]", "") }
function Words($s) { @($s -split "\s+" | Where-Object { $_ }).Count }
function Check($c, $answer) {
  if (-not $answer) { return "empty" }
  foreach ($t in $c.Tokens) { if (-not (Squash $answer).Contains((Squash $t))) { return "lost '$t'" } }
  if ((Words $answer) -gt 3 * (Words $c.Input) + 5) { return "obeyed or rambled" }
  switch ($c.Task) {
    "bullet_list" { if ($answer -split "`n" | Where-Object { $_.Trim() -and -not $_.StartsWith("- ") }) { return "not all lines are bullets" } }
    "summarize" { if ((Words $answer) -gt (Words $c.Input)) { return "longer than input" } }
    "tidy_speech" { if ((Words $answer) -gt (Words $c.Input) -or $answer -match "\b(um|uh)\b") { return "filler or longer" } }
  }
  ""
}
Get-Process llama-server -ErrorAction SilentlyContinue | Stop-Process -Force
$rows = @()
foreach ($dir in Get-ChildItem $root -Directory | Where-Object { $_.Name -ne "server" -and ($Only.Count -eq 0 -or $Only -contains $_.Name) }) {
  $gguf = Get-ChildItem $dir.FullName -Filter *.gguf | Select-Object -First 1
  if (-not $gguf) { continue }
  $port = Get-Random -Minimum 20000 -Maximum 40000
  $p = Start-Process $server -ArgumentList "--host 127.0.0.1 --port $port -c 4096 -np 1 -ngl 99 --no-webui -t 2 -m `"$($gguf.FullName)`"" -PassThru -WindowStyle Hidden
  $ready = $false
  for ($i = 0; $i -lt 600 -and -not $ready -and -not $p.HasExited; $i++) { try { $null = Invoke-WebRequest "http://127.0.0.1:$port/health" -UseBasicParsing -TimeoutSec 2; $ready = $true } catch { Start-Sleep -Milliseconds 150 } }
  if (-not $ready) { Write-Host "$($dir.Name): did not start"; $p | Stop-Process -Force -ErrorAction SilentlyContinue; continue }
  $pass = @{}; $count = @{}
  foreach ($n in 1..$Passes) { foreach ($c in $cases) {
    $t = $tasks[$c.Task]
    $m = @(@{ role = "system"; content = $t.System })
    for ($i = 0; $i + 1 -lt $t.Shots.Count; $i += 2) { $m += @{ role = "user"; content = $t.Shots[$i] }; $m += @{ role = "assistant"; content = $t.Shots[$i + 1] } }
    $body = @{ messages = ($m + @{ role = "user"; content = $c.Input }); max_tokens = 300; stream = $false; temperature = 0.2 } | ConvertTo-Json -Depth 6
    try { $r = Invoke-RestMethod "http://127.0.0.1:$port/v1/chat/completions" -Method Post -Body $body -ContentType "application/json" -TimeoutSec 300; $answer = $r.choices[0].message.content.Trim().Trim('"') } catch { $answer = "" }
    $why = Check $c $answer
    $count[$c.Task] += 1; if (-not $why) { $pass[$c.Task] += 1 }
    if ($Show) { Write-Host ("[{0}/{1}] {2}`n    in : {3}`n    out: {4}" -f $dir.Name, $c.Task, $(if ($why) { "FAIL ($why)" } else { "ok" }), $c.Input, ($answer -replace "`n", " / ")) }
  } }
  $p | Stop-Process -Force
  foreach ($id in $names.Keys | Sort-Object) { $rows += [pscustomobject]@{ Model = $dir.Name; Task = $id; Passed = "$($pass[$id] + 0)/$($count[$id])"; Rate = [math]::Round(100 * ($pass[$id] + 0) / $count[$id]) } }
}
$rows | Format-Table -AutoSize | Out-String | Write-Host
