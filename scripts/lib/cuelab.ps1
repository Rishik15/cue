# File: scripts/lib/cuelab.ps1
# Purpose: the lab scripts' one door into the app: run `cue.exe --lab <command>` and return its parsed answer, so prompts, the guard sentence, answer judging, input cleaning and the installed-model list come from the release exe, never from Rust source.
# Contents: Invoke-CueLab (run one command, stdout as UTF-8, throw on failure), Assert-CueCount (the result count must equal the input count), Write-CueJson (a temp UTF-8 JSON file without BOM), Invoke-CueBatch (one JSON file in, one result per input out), Get-CueTasks, Get-CueClean, Test-CueAnswers, Get-CueInstalled.
# Usage: . "$PSScriptRoot\..\lib\wait.ps1"; . "$PSScriptRoot\..\lib\cuelab.ps1" (load it after wait.ps1). The release exe has no console, so its output is redirected to temp files.

# Runs `cue.exe --lab` with the given words and waits for it; each word is quoted, trailing backslashes doubled so a folder path keeps its closing quote. Inputs: the exe path and the words. Returns stdout read as UTF-8; throws naming the exe when it is missing, and with the stderr text on a non-zero exit.
function Invoke-CueLab($exe, [string[]]$words) {
  if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "$exe not found. Build a release exe first (scripts\cue.ps1 release)" }
  $out = [IO.Path]::GetTempFileName(); $err = [IO.Path]::GetTempFileName()
  try {
    $quoted = @("--lab") + $words | ForEach-Object { '"' + ($_ -replace '(\\+)$', '$1$1') + '"' }
    $p = Start-Process -FilePath $exe -ArgumentList ($quoted -join " ") -Wait -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
    if ($p.ExitCode -ne 0) { throw "cue.exe --lab $($words -join ' ') exited $($p.ExitCode): $([IO.File]::ReadAllText($err, [Text.Encoding]::UTF8).Trim())" }
    [IO.File]::ReadAllText($out, [Text.Encoding]::UTF8)
  } finally { Remove-Item -LiteralPath $out, $err -ErrorAction SilentlyContinue }
}

# Throws when a lab command returned a different number of results than it was given. Inputs: the results, the input count, the command name. Returns the results unchanged as one array.
function Assert-CueCount($results, [int]$expected, $what) {
  $list = @($results)
  if ($list.Count -ne $expected) { throw "cue.exe --lab $what returned $($list.Count) results for $expected inputs" }
  , $list
}

# Writes a value as JSON to a new temp file in UTF-8 without BOM. Input: the value (an array stays an array). Returns the file path.
function Write-CueJson($value) {
  $path = [IO.Path]::GetTempFileName()
  [IO.File]::WriteAllText($path, (ConvertTo-Json -InputObject $value -Depth 4 -Compress), (New-Object Text.UTF8Encoding $false))
  $path
}

# Runs a command that takes a JSON file of inputs and returns one result per input. Inputs: the exe, the command, the inputs. Returns the parsed results as one array.
function Invoke-CueBatch($exe, $command, $inputs) {
  $list = @($inputs)
  if ($list.Count -eq 0) { return , @() }
  $file = Write-CueJson $list
  try { $parsed = ConvertFrom-Json (Invoke-CueLab $exe @($command, $file)) } finally { Remove-Item -LiteralPath $file -ErrorAction SilentlyContinue }
  Assert-CueCount $parsed $list.Count $command
}

# Reads every model task from the app. Input: the exe. Returns an object with `guard` and `tasks.<action id>` holding `system` and the flat `shots` (in, out, in, out, ...).
function Get-CueTasks($exe) { ConvertFrom-Json (Invoke-CueLab $exe @("dump-tasks")) }

# Strips hidden characters the way Cue does before the model sees text. Inputs: the exe and the strings. Returns the cleaned strings in the same order.
function Get-CueClean($exe, $strings) { Invoke-CueBatch $exe "clean" $strings }

# Judges model answers with Cue's own answer check. Inputs: the exe and rows of @{ action; input; answer } (input already cleaned). Returns one { accepted, reason, text } per row, in order.
function Test-CueAnswers($exe, $rows) { Invoke-CueBatch $exe "judge" $rows }

# Lists the speech models installed in a models folder by Cue's own rule. Inputs: the exe and the folder. Returns the model ids.
function Get-CueInstalled($exe, $modelsDir) { @((Invoke-CueLab $exe @("installed", $modelsDir)) -split "`r?`n" | Where-Object { $_ }) }
