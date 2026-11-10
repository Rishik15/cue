# File: scripts/lab/text-latency-bench.ps1
# Purpose: measure how fast and how heavy each text model is end to end, the way Cue runs it: readiness (first start and a second start), time to first token and tokens per second from a streamed request, the time of a whole edit of 1, 2 and 3 sentences, the time of a whole edit of about 500, 2,000 and 6,000 characters (to choose the editing limit EDIT_TOTAL_CHARS in text/budget.rs from numbers), the latency of the model actions that have no reference answer (explain, reply, action items, translate), peak process-tree private memory plus GPU memory, and the unload check (server process gone, memory back near where it was).
# Contents: source readers (tasks from actions/model.rs and model_extra.rs), TSV and escape decoding, Pct, Messages, Stream (a streamed request with first-token time), Edit (sentence by sentence like the pipeline), the per-model loop, the report and the CSV log.
# Usage: pwsh scripts/lab/text-latency-bench.ps1 [-Only id,id] [-Runs 5] [-Ngl 99] [-Ctx 4096] [-Label x] [-Show]
# Limits, stated so the numbers are read correctly: the reply checks of Cue take microseconds (see the Rust tests), so an edit here is the sum of its model requests, not the checks. "Cold" is the first start of the run, so the model file may already sit in the OS cache; a truly cold start needs a reboot or a cleared standby list. Unmeasured until a person runs this on Windows.
param([string[]]$Only = @(), [int]$Runs = 5, [int]$Ngl = 99, [int]$Ctx = 4096, [string]$Label = "", [switch]$Show)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
. (Join-Path $PSScriptRoot "..\lib\textmem.ps1")
$root = Join-Path $env:LOCALAPPDATA "dev.cue.app\models\text"
$server = Join-Path $root "server\llama-server.exe"
$srcDir = Join-Path $PSScriptRoot "..\..\src-tauri\src\actions"
$goldenDir = Join-Path $PSScriptRoot "..\..\bench\golden"
$src = (Get-Content (Join-Path $srcDir "model.rs") -Raw) + "`n" + (Get-Content (Join-Path $srcDir "model_extra.rs") -Raw)
$guard = [regex]::Match($src, 'macro_rules! guard \{.*?"(.*?)"\s*\}', "Singleline").Groups[1].Value
$statics = [ordered]@{ correct = "CORRECT"; explain_simply = "EXPLAIN"; write_reply = "REPLY"; find_tasks = "ACTION_ITEMS"; translate_english = "TRANSLATE" }
$sentences = @(
  "she dont like apples and him have a orange.", "We goed to the market yesterday and buyed some bread.", "Their going to send the invoice befor friday.",
  "I seen the report but it were not finished.", "Please call me at 555-1234 when you recieve this.", "The team have decided to move the meeting to thursday.",
  "Its important that we dont forget the deadline.", "He dont know where the keys is.", "Thanks for you're help with the presentaton.", "Can you check the numbers agian before we send them."
)
# Turns Rust string escapes (\" and \n) into real characters. Input: the escaped text.
function Unescape($s) { $s.Replace('\"', '"').Replace('\n', "`n") }
# Returns every double-quoted Rust string literal in a block, unescaped. Input: the source block.
function Literals($block) { [regex]::Matches($block, '"((?:[^"\\]|\\.)*)"') | ForEach-Object { Unescape $_.Groups[1].Value } }
# Reads one task from the Rust source. Input: the static's name. Returns its system text and example turns.
function ReadTask($static) {
  $block = [regex]::Match($src, "static $static`: Task = Task \{(.*?)\n\};", "Singleline").Groups[1].Value
  $sys = [regex]::Match($block, 'system: concat!\((.*?)\n    \),', "Singleline").Groups[1].Value
  $shots = [regex]::Match($block, 'shots: &\[(.*?)\n    \],', "Singleline").Groups[1].Value
  @{ System = ((Literals ($sys -replace 'guard!\(\)', '')) -join "") + $(if ($sys -match 'guard!\(\)') { $guard } else { "" }); Shots = @(Literals $shots) }
}
# Decodes the \n and \uXXXX escapes of a golden cell. Input: the cell.
function Decode($s) { $t = $s.Replace('\n', "`n"); [regex]::Replace($t, '\\u([0-9A-Fa-f]{4})', { param($m) [string][char][Convert]::ToInt32($m.Groups[1].Value, 16) }) }
# Returns the value at a percentile of a list of numbers. Inputs: the values and the percentile (0 to 100). Returns $null for an empty list.
function Pct($values, $pct) { $s = @($values | Sort-Object); if (-not $s) { return $null }; $s[[math]::Min($s.Count - 1, [int][math]::Ceiling($pct / 100 * $s.Count) - 1)] }
# Builds the chat turns of a task: system, example pairs, then the text. Inputs: the task and the text.
function Messages($task, $text) {
  $m = @(@{ role = "system"; content = $task.System })
  for ($i = 0; $i + 1 -lt $task.Shots.Count; $i += 2) { $m += @{ role = "user"; content = $task.Shots[$i] }; $m += @{ role = "assistant"; content = $task.Shots[$i + 1] } }
  $m + @{ role = "user"; content = $text }
}
# Sends one streamed request. Inputs: the port, the chat turns and the token limit. Returns the answer text, the milliseconds to the first token, the total milliseconds and the tokens per second after the first token (one stream chunk is one token).
function Stream($port, $messages, $maxTokens) {
  $body = @{ messages = $messages; max_tokens = $maxTokens; stream = $true; cache_prompt = $true; temperature = 0; top_k = 1 } | ConvertTo-Json -Depth 6
  $client = [System.Net.Http.HttpClient]::new(); $client.Timeout = [TimeSpan]::FromSeconds(300)
  $request = [System.Net.Http.HttpRequestMessage]::new([System.Net.Http.HttpMethod]::Post, "http://127.0.0.1:$port/v1/chat/completions")
  $request.Content = [System.Net.Http.StringContent]::new($body, [Text.Encoding]::UTF8, "application/json")
  $clock = [Diagnostics.Stopwatch]::StartNew()
  $response = $client.SendAsync($request, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
  $reader = [IO.StreamReader]::new($response.Content.ReadAsStreamAsync().GetAwaiter().GetResult())
  $first = $null; $chunks = 0; $text = [Text.StringBuilder]::new()
  while ($null -ne ($line = $reader.ReadLine())) {
    if (-not $line.StartsWith("data: ") -or $line -eq "data: [DONE]") { continue }
    $delta = ($line.Substring(6) | ConvertFrom-Json).choices[0].delta.content
    if ($delta) { if ($null -eq $first) { $first = $clock.Elapsed.TotalMilliseconds }; $chunks++; [void]$text.Append($delta) }
  }
  $total = $clock.Elapsed.TotalMilliseconds
  $client.Dispose()
  $rate = if ($chunks -gt 1 -and $total -gt $first) { ($chunks - 1) / (($total - $first) / 1000) } else { $null }
  @{ Text = $text.ToString(); FirstMs = $first; TotalMs = $total; Tps = $rate }
}
# Edits sentence by sentence like the pipeline does. Inputs: the port, the task and the sentences. Returns the total milliseconds.
function Edit($port, $task, $list) {
  $clock = [Diagnostics.Stopwatch]::StartNew()
  foreach ($s in $list) { $null = Stream $port (Messages $task $s) ([int]($s.Length / 2) + 80) }
  $clock.Elapsed.TotalMilliseconds
}
# Returns about `chars` characters of the sample sentences, whole sentences only. Input: the target length.
function TextOf($chars) {
  $list = @(); $length = 0; $i = 0
  while ($length -lt $chars) { $s = $sentences[$i % $sentences.Count]; $list += $s; $length += $s.Length + 1; $i++ }
  , $list
}
# Starts a server like Cue does. Inputs: the model file and the port. Returns the process and the seconds until it answered /health, or $null when it did not start.
function Start-Model($gguf, $port) {
  $started = Get-Date
  $p = Start-Process $server -ArgumentList "--host 127.0.0.1 --port $port -c $Ctx -np 1 -ngl $Ngl --no-webui -t 2 -m `"$gguf`"" -PassThru -WindowStyle Hidden
  $ready = $null -ne (WaitUntil { $p.HasExited -or $(try { $null = Invoke-WebRequest "http://127.0.0.1:$port/health" -UseBasicParsing -TimeoutSec 2; $true } catch { $false }) } 120000) -and -not $p.HasExited
  if (-not $ready) { $p | Stop-Process -Force -ErrorAction SilentlyContinue; return $null }
  @{ Process = $p; Seconds = ((Get-Date) - $started).TotalSeconds }
}
# Returns the free physical memory in MB.
function FreeMB { [math]::Round((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1KB) }

Get-Process llama-server -ErrorAction SilentlyContinue | Stop-Process -Force
$task = @{}; foreach ($id in $statics.Keys) { $task[$id] = ReadTask $statics[$id] }
$rows = @()
foreach ($dir in Get-ChildItem $root -Directory | Where-Object { $_.Name -ne "server" -and ($Only.Count -eq 0 -or $Only -contains $_.Name) }) {
  $gguf = Get-ChildItem $dir.FullName -Filter *.gguf | Select-Object -First 1
  if (-not $gguf) { continue }
  $port = Get-Random -Minimum 20000 -Maximum 40000
  $freeBefore = FreeMB
  $cold = Start-Model $gguf.FullName $port
  if (-not $cold) { Write-Host "$($dir.Name): did not start"; continue }
  $cold.Process | Stop-Process -Force; $null = $cold.Process.WaitForExit(10000)
  $warm = Start-Model $gguf.FullName $port
  if (-not $warm) { Write-Host "$($dir.Name): did not restart"; continue }
  $p = $warm.Process; $peakMb = 0; $peakGpu = 0
  $first = @(); $tps = @(); $one = @(); $two = @(); $three = @(); $size = @{ 500 = @(); 2000 = @(); 6000 = @() }; $action = @{}
  $null = Stream $port (Messages $task.correct "warm up the prefix.") 8
  foreach ($n in 1..$Runs) {
    $s = Stream $port (Messages $task.correct $sentences[$n % $sentences.Count]) 80
    $first += $s.FirstMs; if ($s.Tps) { $tps += $s.Tps }
    $one += Edit $port $task.correct $sentences[0..0]; $two += Edit $port $task.correct $sentences[0..1]; $three += Edit $port $task.correct $sentences[0..2]
    foreach ($chars in @($size.Keys)) { $list = TextOf $chars; $size[$chars] += Edit $port $task.correct $list }
    $peakMb = [math]::Max($peakMb, (TreeMemoryMB $p.Id)); $peakGpu = [math]::Max($peakGpu, (GpuMemoryMB $p.Id))
  }
  foreach ($id in @("explain_simply", "write_reply", "find_tasks", "translate_english")) {
    $inputs = @(Get-Content (Join-Path $goldenDir "$id.tsv") -Encoding UTF8 | ForEach-Object { Decode (($_ -split "`t")[1]) } | Where-Object { $_ } | Select-Object -First 3)
    $times = @()
    foreach ($text in $inputs) { foreach ($n in 1..[math]::Max(1, [int]($Runs / 2))) { $s = Stream $port (Messages $task[$id] $text) 384; $times += $s.TotalMs; if ($Show -and $n -eq 1) { Write-Host ("  [{0}/{1}] {2}" -f $dir.Name, $id, $s.Text) } } }
    $action[$id] = $times
    $peakMb = [math]::Max($peakMb, (TreeMemoryMB $p.Id)); $peakGpu = [math]::Max($peakGpu, (GpuMemoryMB $p.Id))
  }
  $p | Stop-Process -Force; $null = $p.WaitForExit(10000)
  $gone = $null -eq (Get-Process llama-server -ErrorAction SilentlyContinue)
  $freeAfter = FreeMB
  $row = [ordered]@{
    Label = $Label; Model = $dir.Name; ColdSec = [math]::Round($cold.Seconds, 1); WarmSec = [math]::Round($warm.Seconds, 1)
    FirstTokenP50 = [math]::Round((Pct $first 50)); FirstTokenP95 = [math]::Round((Pct $first 95)); TokPerSec = [math]::Round((Pct $tps 50), 1)
    Edit1P50 = [math]::Round((Pct $one 50)); Edit1P95 = [math]::Round((Pct $one 95)); Edit2P50 = [math]::Round((Pct $two 50)); Edit2P95 = [math]::Round((Pct $two 95)); Edit3P50 = [math]::Round((Pct $three 50)); Edit3P95 = [math]::Round((Pct $three 95))
    Chars500P50 = [math]::Round((Pct $size[500] 50)); Chars2000P50 = [math]::Round((Pct $size[2000] 50)); Chars6000P50 = [math]::Round((Pct $size[6000] 50))
    ExplainP50 = [math]::Round((Pct $action.explain_simply 50)); ReplyP50 = [math]::Round((Pct $action.write_reply 50)); TasksP50 = [math]::Round((Pct $action.find_tasks 50)); TranslateP50 = [math]::Round((Pct $action.translate_english 50))
    MemoryMB = $peakMb; GpuMB = $peakGpu; Unloaded = $gone; FreeBeforeMB = $freeBefore; FreeAfterMB = $freeAfter; Ngl = $Ngl; Ctx = $Ctx
  }
  $rows += [pscustomobject]$row
}
$rows | Format-Table Label, Model, ColdSec, WarmSec, FirstTokenP50, FirstTokenP95, TokPerSec, Edit1P50, Edit2P50, Edit3P50, Edit3P95, Chars500P50, Chars2000P50, Chars6000P50, MemoryMB, GpuMB, Unloaded -AutoSize | Out-String | Write-Host
$rows | Format-Table Label, Model, ExplainP50, ReplyP50, TasksP50, TranslateP50, FreeBeforeMB, FreeAfterMB -AutoSize | Out-String | Write-Host
$rows | Export-Csv (Join-Path $env:TEMP "text-latency-log.csv") -Append -NoTypeInformation
