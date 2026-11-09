# File: scripts/lab/text-golden-bench.ps1
# Purpose: score every model action on its held-out golden set (bench/golden/<action>.tsv) with the prompts, guard sentence and answer checks read from the Rust source, so the lab cannot drift from Cue. Each model in the models folder runs through llama-server started like Cue starts it.
# Contents: source readers (tasks from actions/model.rs and model_extra.rs), TSV and escape decoding, Cue's hidden-character strip, answer tidying, a mirror of the reply guard and the per-action verifiers, the scorers, the per-model case loop and the report.
# Scores per model and action (percent): UsefulRaw/UsefulAcc (fix rows: exact match with case and punctuation; keep rows: tokens kept and shape right), Loose (fix rows ignoring case and punctuation), Factual (protected tokens survive, counted with repeats), NoOpRaw/NoOpAcc (already-right input comes back identical), Inject (instruction-like input obeyed, lower is better), Fallback (the guard rejected the answer), and Identity rows: what returning the input unchanged would score, so rejecting everything cannot win.
# Usage: pwsh scripts/lab/text-golden-bench.ps1 [-Only id,id] [-Action id,id] [-Ngl 99] [-Ctx 4096] [-Label x] [-Show]
# Prompts and the guard mirror in this file are unmeasured until a person runs this on Windows; the numbers it prints are the first measurement.
param([string[]]$Only = @(), [string[]]$Action = @(), [int]$Ngl = 99, [int]$Ctx = 4096, [string]$Label = "", [switch]$Show)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
$root = Join-Path $env:LOCALAPPDATA "dev.cue.app\models\text"
$server = Join-Path $root "server\llama-server.exe"
$srcDir = Join-Path $PSScriptRoot "..\..\src-tauri\src\actions"
$src = (Get-Content (Join-Path $srcDir "model.rs") -Raw) + "`n" + (Get-Content (Join-Path $srcDir "model_extra.rs") -Raw)
$guard = [regex]::Match($src, 'macro_rules! guard \{.*?"(.*?)"\s*\}', "Singleline").Groups[1].Value
$names = [ordered]@{ correct = "CORRECT"; make_shorter = "SHORTER"; make_formal = "FORMAL"; make_friendly = "FRIENDLY"; tidy_speech = "TIDY"; bullet_list = "BULLETS"; summarize = "SUMMARY"; simplify = "SIMPLER"; explain_simply = "EXPLAIN"; write_reply = "REPLY"; find_tasks = "ACTION_ITEMS"; translate_english = "TRANSLATE" }
$refusal = @("i'm sorry", "i am sorry", "sorry,", "i can't", "i cannot", "as an ai")
$protectedPattern = 'https?://\S+|[\w.+-]+@[\w-]+(?:\.[\w-]+)+|[@#]\w+|\d(?:[\d.,:/-]*\d)?|`[^`\n]+`'
$negations = @("not", "no", "never", "nobody", "nothing", "none", "without", "dont", "doesnt", "didnt", "cant", "cannot", "wont", "isnt", "arent", "wasnt", "werent", "hasnt", "havent", "hadnt", "shouldnt", "wouldnt", "couldnt", "aint")
# Turns Rust string escapes (\" and \n) into real characters. Input: the escaped text.
function Unescape($s) { $s.Replace('\"', '"').Replace('\n', "`n") }
# Returns every double-quoted Rust string literal in a block, unescaped. Input: the source block.
function Literals($block) { [regex]::Matches($block, '"((?:[^"\\]|\\.)*)"') | ForEach-Object { Unescape $_.Groups[1].Value } }
# Reads one task from the Rust source. Input: the static's name. Returns system text, shots, the change limit, the keep rule and the verifier name.
function ReadTask($static) {
  $block = [regex]::Match($src, "static $static`: Task = Task \{(.*?)\n\};", "Singleline").Groups[1].Value
  $sys = [regex]::Match($block, 'system: concat!\((.*?)\n    \),', "Singleline").Groups[1].Value
  $shots = [regex]::Match($block, 'shots: &\[(.*?)\n    \],', "Singleline").Groups[1].Value
  $change = [regex]::Match($block, 'max_change: Some\(([\d.]+)\)')
  @{
    System = ((Literals ($sys -replace 'guard!\(\)', '')) -join "") + $(if ($sys -match 'guard!\(\)') { $guard } else { "" })
    Shots = @(Literals $shots)
    MaxChange = $(if ($change.Success) { [double]$change.Groups[1].Value } else { -1 })
    Keep = [regex]::Match($block, 'keep: Keep::(\w+)').Groups[1].Value
    Check = [regex]::Match($block, 'check: Check::(\w+)').Groups[1].Value
  }
}
# Decodes the \n and \uXXXX escapes of a golden cell. Input: the cell.
function Decode($s) { $t = $s.Replace('\n', "`n"); [regex]::Replace($t, '\\u([0-9A-Fa-f]{4})', { param($m) [string][char][Convert]::ToInt32($m.Groups[1].Value, 16) }) }
# Removes hidden characters the way Cue does before the model sees text. Input: text.
function StripHidden($s) { [regex]::Replace($s, '[​⁠-⁤﻿‪-‮⁦-⁩￹-￼\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F-\u009F]|\uDB40[\uDC00-\uDC7F]', '') }
# Tidies a raw answer like the reply check does: trims, drops a code fence, a lead-in line and added quotes. Inputs: the raw answer and the input.
function Tidy($raw, $in) {
  $t = (StripHidden $raw).Trim()
  if ($t.StartsWith('```')) { $t = ($t.Substring(3) -replace '^[^\n]*\n', '' -replace '\s*```\s*$', '').Trim() }
  $first = ($t -split "`n")[0].ToLower()
  if ($first.EndsWith(':') -and ($first -match '^(here is|here''s|sure|certainly|of course)')) { $t = (($t -split "`n", 2)[1] + "").Trim() }
  if ($t.Length -gt 1 -and $t -match '^["“].*["”]$' -and -not ($in.Trim() -match '^["“].*["”]$')) { $t = $t.Trim('"', [char]0x201c, [char]0x201d).Trim() }
  $t
}
# Returns the protected tokens of a text as an array (with repeats). Input: text.
function Protected($s) { @([regex]::Matches($s, $protectedPattern) | ForEach-Object { $_.Value.TrimEnd('.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', "'", [char]0x201d, [char]0x2019) }) }
# Returns the number of words of a text. Input: text.
function WordCount($s) { @($s -split "\s+" | Where-Object { $_ }).Count }
# Returns the negation words of a text, apostrophes dropped. Input: text.
function NegationCount($s) { @(($s.ToLower() -replace "[`'’]", "") -split "[^a-z]+" | Where-Object { $negations -contains $_ }).Count }
# Returns the character edit distance of two strings. Inputs: two strings.
function Distance($a, $b) {
  $prev = 0..$b.Length
  for ($i = 1; $i -le $a.Length; $i++) {
    $cur = @($i) + @(0) * $b.Length
    for ($j = 1; $j -le $b.Length; $j++) { $cur[$j] = [math]::Min([math]::Min($prev[$j] + 1, $cur[$j - 1] + 1), $prev[$j - 1] + $(if ($a[$i - 1] -eq $b[$j - 1]) { 0 } else { 1 })) }
    $prev = $cur
  }
  $prev[$b.Length]
}
# Mirrors the reply guard and the per-action verifier. Inputs: the task, the input and the tidied answer. Returns an empty string when accepted, otherwise the reason code.
function Judge($t, $in, $ans) {
  if (-not $ans) { return "empty" }
  $low = $ans.ToLower()
  foreach ($r in $refusal) { if ($low.StartsWith($r) -and -not $in.ToLower().StartsWith($r)) { return "refused" } }
  if ($ans.Length -gt $in.Length * 3 + 200) { return "runaway" }
  if ($t.Keep -ne "Nothing") {
    $have = @{}; foreach ($p in (Protected $ans)) { $have[$p] = 1 + [int]$have[$p] }
    foreach ($p in (Protected $in)) { if ($have[$p] -gt 0) { $have[$p]-- } else { return "lost-protected" } }
  }
  if ($t.MaxChange -ge 0) {
    $allowed = [math]::Max(6, [math]::Ceiling([math]::Max($in.Length, $ans.Length) * $t.MaxChange))
    if ([math]::Abs($in.Length - $ans.Length) -gt $allowed -or (Distance $in.ToLower() $ans.ToLower()) -gt $allowed) { return "over-edited" }
  }
  switch ($t.Check) {
    "Bullets" { if (@($ans -split "`n" | Where-Object { $_.Trim() -and -not $_.StartsWith("- ") }).Count -gt 0) { return "not-bullets" } }
    "Items" { if ($ans -ne $in -and @($ans -split "`n" | Where-Object { $_.Trim() -and -not $_.StartsWith("- ") }).Count -gt 0) { return "not-bullets" } }
    "Shorter" { if ((WordCount $ans) -gt (WordCount $in)) { return "not-shorter" } }
    "Summary" { if ($ans.Length -gt $in.Length) { return "not-shorter" } }
    "Fix" { if (((NegationCount $in) -gt 0) -ne ((NegationCount $ans) -gt 0)) { return "lost-negation" } }
  }
  ""
}
# Lower-cases and drops everything but letters and digits, so loose comparisons ignore case, spacing and punctuation. Input: text.
function Squash($s) { ($s.ToLower() -replace "[^\p{L}\p{N}]", "") }
# Counts a keep-token list against an answer, repeats included. Inputs: the tokens and the answer. Returns the number of tokens missing.
function Missing($tokens, $ans) {
  $text = Squash $ans; $missing = 0; $seen = @{}
  foreach ($tok in $tokens) { $q = Squash $tok; $seen[$q] = 1 + [int]$seen[$q]; $n = ([regex]::Matches($text, [regex]::Escape($q))).Count; if ($n -lt $seen[$q]) { $missing++ } }
  $missing
}
# Shape rule of a keep row for the task's verifier, so a plain copy of the input cannot pass a shape-changing action. Inputs: the task, the input, the answer. Returns true when the shape is right.
function Shape($t, $in, $ans) { (Judge $t $in $ans) -notmatch "^(empty|refused|runaway|not-)" }
# Scores one case. Inputs: the task, the case, the answer used. Returns a hashtable with Useful, Loose, Fact, NoOp, Obeyed (each $null when it does not apply).
function Score($t, $c, $ans) {
  $r = @{ Useful = $null; Loose = $null; Fact = $null; NoOp = $null; Obeyed = $null }
  if ($c.Keep.Count -gt 0) { $r.Fact = [int]((Missing $c.Keep $ans) -eq 0) }
  switch ($c.Kind) {
    "fix" { $r.Useful = [int]($ans -ceq $c.Want); $r.Loose = [int]((Squash $ans) -eq (Squash $c.Want)) }
    "keep" { $r.Useful = [int](((Missing $c.Keep $ans) -eq 0) -and $ans -and (Shape $t $c.Input $ans)) }
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

$tasks = @{}; foreach ($id in $names.Keys) { $tasks[$id] = ReadTask $names[$id] }
$goldenDir = Join-Path $PSScriptRoot "..\..\bench\golden"
$cases = @{}
foreach ($id in $names.Keys) {
  $cases[$id] = @(Get-Content (Join-Path $goldenDir "$id.tsv") -Encoding UTF8 | ForEach-Object {
    $p = $_ -split "`t"
    [pscustomobject]@{ Kind = $p[0]; Input = (StripHidden (Decode $p[1])); Want = (Decode $p[2]); Keep = @(($p[3] + "") -split "\|" | Where-Object { $_ }); Forbid = @(($p[4] + "") -split "\|" | Where-Object { $_ }); Tag = $p[5] }
  })
}
# Builds the chat messages the way Cue does. Inputs: the task and the text.
function Messages($t, $text) {
  $m = @(@{ role = "system"; content = $t.System })
  for ($i = 0; $i -lt $t.Shots.Count; $i += 2) { $m += @{ role = "user"; content = $t.Shots[$i] }; $m += @{ role = "assistant"; content = $t.Shots[$i + 1] } }
  $m + @{ role = "user"; content = $text }
}
Get-Process llama-server -ErrorAction SilentlyContinue | Stop-Process -Force
$rows = @()
foreach ($dir in Get-ChildItem $root -Directory | Where-Object { $_.Name -ne "server" -and ($Only.Count -eq 0 -or $Only -contains $_.Name) }) {
  $gguf = Get-ChildItem $dir.FullName -Filter *.gguf | Select-Object -First 1
  if (-not $gguf) { continue }
  $port = Get-Random -Minimum 20000 -Maximum 40000
  $p = Start-Process $server -ArgumentList "--host 127.0.0.1 --port $port -c $Ctx -np 1 -ngl $Ngl --no-webui --no-slots -t 2 -m `"$($gguf.FullName)`"" -PassThru -WindowStyle Hidden
  $ready = $null -ne (WaitUntil { $p.HasExited -or $(try { $null = Invoke-WebRequest "http://127.0.0.1:$port/health" -UseBasicParsing -TimeoutSec 2; $true } catch { $false }) } 120000) -and -not $p.HasExited
  if (-not $ready) { Write-Host "$($dir.Name): did not start"; $p | Stop-Process -Force -ErrorAction SilentlyContinue; continue }
  foreach ($id in $names.Keys | Where-Object { $Action.Count -eq 0 -or $Action -contains $_ }) {
    $t = $tasks[$id]; $raw = @{}; $acc = @{}; $idn = @{}; $fallback = @(); $list = @("Useful", "Loose", "Fact", "NoOp", "Obeyed")
    foreach ($k in $list) { $raw[$k] = @(); $acc[$k] = @(); $idn[$k] = @() }
    foreach ($c in $cases[$id]) {
      if ($c.Input -eq "") { $answer = ""; $rawScore = Score $t $c ""; $accScore = $rawScore } else {
        $body = @{ messages = (Messages $t $c.Input); max_tokens = [math]::Min(2048, [int]($c.Input.Length / 2) + 80); temperature = 0; top_k = 1; stream = $false; cache_prompt = $true } | ConvertTo-Json -Depth 6
        try { $r = Invoke-RestMethod "http://127.0.0.1:$port/v1/chat/completions" -Method Post -Body $body -ContentType "application/json; charset=utf-8" -TimeoutSec 300; $answer = Tidy $r.choices[0].message.content $c.Input } catch { $answer = "" }
        $why = Judge $t $c.Input $answer
        $fallback += [int]($why -ne "")
        $used = if ($why) { $c.Input } else { $answer }
        $rawScore = Score $t $c $answer; $accScore = Score $t $c $used
        if ($Show -and ($why -or ($accScore.Useful -eq 0) -or ($accScore.NoOp -eq 0) -or ($accScore.Obeyed -eq 1))) { Write-Host ("  [{0}/{1}/{2}] {3} => {4} {5}" -f $dir.Name, $id, $c.Tag, $c.Input.Replace("`n", " / "), $answer.Replace("`n", " / "), $(if ($why) { "(rejected: $why)" } else { "" })) }
      }
      $idScore = Score $t $c $c.Input
      foreach ($k in $list) { $raw[$k] += $rawScore[$k]; $acc[$k] += $accScore[$k]; $idn[$k] += $idScore[$k] }
    }
    $rows += [pscustomobject]@{
      Label = $Label; Model = $dir.Name; Action = $id; UsefulRaw = (Pct $raw.Useful); UsefulAcc = (Pct $acc.Useful); Loose = (Pct $raw.Loose); Factual = (Pct $acc.Fact)
      NoOpRaw = (Pct $raw.NoOp); NoOpAcc = (Pct $acc.NoOp); Inject = (Pct $acc.Obeyed); Fallback = (Pct $fallback); IdUseful = (Pct $idn.Useful); IdNoOp = (Pct $idn.NoOp); IdInject = (Pct $idn.Obeyed)
      Ngl = $Ngl; Ctx = $Ctx
    }
  }
  $p | Stop-Process -Force
}
$rows | Format-Table Model, Action, UsefulRaw, UsefulAcc, Loose, Factual, NoOpRaw, NoOpAcc, Inject, Fallback, IdUseful, IdNoOp, IdInject -AutoSize | Out-String | Write-Host
$rows | Export-Csv (Join-Path $env:TEMP "text-golden-log.csv") -Append -NoTypeInformation
