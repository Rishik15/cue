# File: scripts/lab/text-golden-bench.ps1
# Purpose: score every model action on its held-out golden set (bench/golden/<action>.tsv) with the prompts and guard sentence production sends and the answer check production runs, all read from the release exe (`cue.exe --lab`), so the lab cannot drift from Cue. Each model in the models folder runs through llama-server started like Cue starts it.
# Contents: TSV and escape decoding, the scorers, Messages, Ask, the per-model case loop (answers of one action judged by Cue in one batch; servers start through scripts/lib/llama.ps1 and its finally stops every server it started, even on Ctrl+C) and the report. Refuses to start while another llama-server runs.
# Scores per model and action (percent): UsefulRaw/UsefulAcc (fix rows: exact match with case and punctuation; keep rows: tokens kept and shape right), Loose (fix rows ignoring case and punctuation), Factual (protected tokens survive, counted with repeats), NoOpRaw/NoOpAcc (already-right input comes back identical), Inject (instruction-like input obeyed, lower is better), Fallback (Cue's answer check rejected the answer), and Identity rows: what returning the input unchanged would score, so rejecting everything cannot win.
# Usage: pwsh scripts/lab/text-golden-bench.ps1 [-Only id,id] [-Action id,id] [-Ngl 99] [-Ctx 4096] [-Label x] [-Show] [-Exe path]   (needs a release exe: scripts\cue.ps1 release)
# Prompts are unmeasured until a person runs this on Windows; the numbers it prints are the first measurement.
param([string[]]$Only = @(), [string[]]$Action = @(), [int]$Ngl = 99, [int]$Ctx = 4096, [string]$Label = "", [switch]$Show, [string]$Exe = (Join-Path $PSScriptRoot "..\..\src-tauri\target\release\cue.exe"))
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\llama.ps1")
. (Join-Path $PSScriptRoot "..\lib\cuelab.ps1")
$root = Join-Path $env:LOCALAPPDATA "dev.cue.app\models\text"
$server = Join-Path $root "server\llama-server.exe"
$ids = @("correct", "make_shorter", "make_formal", "make_friendly", "tidy_speech", "bullet_list", "summarize", "simplify", "explain_simply", "write_reply", "find_tasks", "translate_english")
# Decodes the \n and \uXXXX escapes of a golden cell. Input: the cell.
function Decode($s) { $t = $s.Replace('\n', "`n"); [regex]::Replace($t, '\\u([0-9A-Fa-f]{4})', { param($m) [string][char][Convert]::ToInt32($m.Groups[1].Value, 16) }) }
# Lower-cases and drops everything but letters and digits, so loose comparisons ignore case, spacing and punctuation. Input: text.
function Squash($s) { ($s.ToLower() -replace "[^\p{L}\p{N}]", "") }
# Counts a keep-token list against an answer, repeats included. Inputs: the tokens and the answer. Returns the number of tokens missing.
function Missing($tokens, $ans) {
  $text = Squash $ans; $missing = 0; $seen = @{}
  foreach ($tok in $tokens) { $q = Squash $tok; $seen[$q] = 1 + [int]$seen[$q]; $n = ([regex]::Matches($text, [regex]::Escape($q))).Count; if ($n -lt $seen[$q]) { $missing++ } }
  $missing
}
# Scores one case. Inputs: the case, the answer used and Cue's rejection code for that answer ("" when accepted); a keep row needs the right shape, so a plain copy of the input cannot pass a shape-changing action. Returns a hashtable with Useful, Loose, Fact, NoOp, Obeyed (each $null when it does not apply).
function Score($c, $ans, $reason) {
  $r = @{ Useful = $null; Loose = $null; Fact = $null; NoOp = $null; Obeyed = $null }
  if ($c.Keep.Count -gt 0) { $r.Fact = [int]((Missing $c.Keep $ans) -eq 0) }
  switch ($c.Kind) {
    "fix" { $r.Useful = [int]($ans -ceq $c.Want); $r.Loose = [int]((Squash $ans) -eq (Squash $c.Want)) }
    "keep" { $r.Useful = [int](((Missing $c.Keep $ans) -eq 0) -and $ans -and ("$reason" -notmatch "^(empty|refused|runaway|not-)")) }
    "same" { $r.NoOp = [int]($ans -ceq $c.Input) }
    "inject" {
      $obeyed = ((Missing $c.Keep $ans) -gt 0)
      foreach ($f in $c.Forbid) { if ($ans.ToLower().Contains($f.ToLower())) { $obeyed = $true } }
      $r.Obeyed = [int]$obeyed
    }
  }
  $r
}
# Returns the mean of a list as a percent, or an empty string when the list is empty. Input: numbers or nulls.
function Pct($xs) { $v = @($xs | Where-Object { $null -ne $_ }); if ($v.Count -eq 0) { "" } else { [math]::Round(100 * ($v | Measure-Object -Average).Average, 1) } }

$tasks = (Get-CueTasks $Exe).tasks
$goldenDir = Join-Path $PSScriptRoot "..\..\bench\golden"
$cases = @{}
foreach ($id in $ids) {
  $cases[$id] = @(Get-Content (Join-Path $goldenDir "$id.tsv") -Encoding UTF8 | ForEach-Object {
    $p = $_ -split "`t"
    [pscustomobject]@{ Kind = $p[0]; Input = (Decode $p[1]); Want = (Decode $p[2]); Keep = @(($p[3] + "") -split "\|" | Where-Object { $_ }); Forbid = @(($p[4] + "") -split "\|" | Where-Object { $_ }); Tag = $p[5] }
  })
}
$all = @($ids | ForEach-Object { $cases[$_] })
$clean = Get-CueClean $Exe @($all | ForEach-Object { $_.Input })
for ($i = 0; $i -lt $all.Count; $i++) { $all[$i].Input = $clean[$i] }
# Builds the chat messages the way Cue does. Inputs: the task and the text.
function Messages($t, $text) {
  $m = @(@{ role = "system"; content = $t.System })
  for ($i = 0; $i -lt $t.Shots.Count; $i += 2) { $m += @{ role = "user"; content = $t.Shots[$i] }; $m += @{ role = "assistant"; content = $t.Shots[$i + 1] } }
  $m + @{ role = "user"; content = $text }
}
# Sends one case to the server like Cue does. Inputs: the port, the task and the cleaned input. Returns the raw answer, or "" when the request failed.
function Ask($port, $t, $text) {
  $body = @{ messages = (Messages $t $text); max_tokens = [math]::Min(2048, [int]($text.Length / 2) + 80); temperature = 0; top_k = 1; stream = $false; cache_prompt = $true } | ConvertTo-Json -Depth 6
  try { [string](Invoke-RestMethod "http://127.0.0.1:$port/v1/chat/completions" -Method Post -Body $body -ContentType "application/json; charset=utf-8" -TimeoutSec 300).choices[0].message.content } catch { "" }
}
Assert-NoLlamaServer
$rows = @()
foreach ($dir in Get-ChildItem $root -Directory | Where-Object { $_.Name -ne "server" -and ($Only.Count -eq 0 -or $Only -contains $_.Name) }) {
  $gguf = Get-ChildItem $dir.FullName -Filter *.gguf | Select-Object -First 1
  if (-not $gguf) { continue }
  $port = Get-Random -Minimum 20000 -Maximum 40000
  $owned = [Collections.Generic.List[Diagnostics.Process]]::new()
  try {
    if (-not (Start-LlamaServer $server "--host 127.0.0.1 --port $port -c $Ctx -np 1 -ngl $Ngl --no-webui --no-slots -t 2 -m `"$($gguf.FullName)`"" $port $owned 120000)) { Write-Host "$($dir.Name): did not start"; continue }
    foreach ($id in $ids | Where-Object { $Action.Count -eq 0 -or $Action -contains $_ }) {
      $t = $tasks.$id; $list = @($cases[$id]); $raw = @{}; $acc = @{}; $idn = @{}; $fallback = @(); $keys = @("Useful", "Loose", "Fact", "NoOp", "Obeyed")
      foreach ($k in $keys) { $raw[$k] = @(); $acc[$k] = @(); $idn[$k] = @() }
      $answers = @($list | ForEach-Object { if ($_.Input -eq "") { "" } else { Ask $port $t $_.Input } })
      $judged = Test-CueAnswers $Exe (@(for ($i = 0; $i -lt $list.Count; $i++) { @{ action = $id; input = $list[$i].Input; answer = $answers[$i] } }) + @($list | ForEach-Object { @{ action = $id; input = $_.Input; answer = $_.Input } }))
      for ($i = 0; $i -lt $list.Count; $i++) {
        $c = $list[$i]; $v = $judged[$i]; $same = $judged[$list.Count + $i]; $why = "$($v.reason)"; $answer = $v.text
        $rawScore = Score $c $answer $why; $accScore = $rawScore
        if ($c.Input -ne "") {
          $fallback += [int](-not $v.accepted)
          if (-not $v.accepted) { $accScore = Score $c $c.Input "$($same.reason)" }
          if ($Show -and ($why -or ($accScore.Useful -eq 0) -or ($accScore.NoOp -eq 0) -or ($accScore.Obeyed -eq 1))) { Write-Host ("  [{0}/{1}/{2}] {3} => {4} {5}" -f $dir.Name, $id, $c.Tag, $c.Input.Replace("`n", " / "), $answer.Replace("`n", " / "), $(if ($why) { "(rejected: $why)" } else { "" })) }
        }
        $idScore = Score $c $c.Input "$($same.reason)"
        foreach ($k in $keys) { $raw[$k] += $rawScore[$k]; $acc[$k] += $accScore[$k]; $idn[$k] += $idScore[$k] }
      }
      $rows += [pscustomobject]@{
        Label = $Label; Model = $dir.Name; Action = $id; UsefulRaw = (Pct $raw.Useful); UsefulAcc = (Pct $acc.Useful); Loose = (Pct $raw.Loose); Factual = (Pct $acc.Fact)
        NoOpRaw = (Pct $raw.NoOp); NoOpAcc = (Pct $acc.NoOp); Inject = (Pct $acc.Obeyed); Fallback = (Pct $fallback); IdUseful = (Pct $idn.Useful); IdNoOp = (Pct $idn.NoOp); IdInject = (Pct $idn.Obeyed)
        Ngl = $Ngl; Ctx = $Ctx
      }
    }
  } finally { Stop-LlamaServers $owned }
}
$rows | Format-Table Model, Action, UsefulRaw, UsefulAcc, Loose, Factual, NoOpRaw, NoOpAcc, Inject, Fallback, IdUseful, IdNoOp, IdInject -AutoSize | Out-String | Write-Host
$rows | Export-Csv (Join-Path $env:TEMP "text-golden-log.csv") -Append -NoTypeInformation
