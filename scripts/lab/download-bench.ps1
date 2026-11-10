# File: scripts/lab/download-bench.ps1
# Purpose: measure a model download on Windows. Runs the ignored `download_bench` test (a real HTTPS fetch of one pinned file through Cue's own transfer code) N times and prints per run: first byte, fetch time and rate, tail (fsync and rename), verify pass, retries, bytes on the wire, peak private memory, CPU seconds and threads of the process tree, and the idle state five seconds after the download (threads back to the start count, CPU near zero). Optional -Aria2 times aria2c on the same link for comparison. Rows are appended to %TEMP%\download-bench-log.csv with -Label.
# Contents: the build step, Run-One (one sampled run), the run loop, the median table, the optional aria2 comparison.
# Usage: pwsh scripts/lab/download-bench.ps1 -Url https://... -Sha256 <hex> -Size <bytes> [-Runs 5] [-Label x] [-Aria2 path\aria2c.exe] [-Dir path]
param(
  [Parameter(Mandatory)][string]$Url, [Parameter(Mandatory)][string]$Sha256, [Parameter(Mandatory)][long]$Size,
  [int]$Runs = 5, [string]$Label = "", [string]$Aria2 = "", [string]$Dir = (Join-Path $env:TEMP "cue-dl-bench")
)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\win32.ps1")
. (Join-Path $PSScriptRoot "..\lib\proc.ps1")
Set-Location (Join-Path $PSScriptRoot "..\..")
$manifest = "src-tauri\Cargo.toml"

# Builds the release test binary once and returns its path.
function Build-Bench {
  $json = & cargo test --release --manifest-path $manifest --lib --no-run --message-format=json 2>$null | ForEach-Object { try { $_ | ConvertFrom-Json } catch { $null } }
  if ($LASTEXITCODE -ne 0) { throw "cargo test --no-run failed" }
  $exe = ($json | Where-Object { $_.reason -eq "compiler-artifact" -and $_.executable -and $_.target.kind -contains "lib" -and $_.profile.test } | Select-Object -Last 1).executable
  if (-not $exe) { throw "could not find the test binary" }
  $exe
}

# Reads the BENCH lines a run printed so far. Input: the output file. Returns the lines (empty while the file is not there yet).
function Bench-Lines($path) { if (Test-Path $path) { @(Get-Content $path -ErrorAction SilentlyContinue | Where-Object { $_ -like "BENCH *" }) } else { @() } }

# Runs the bench test once while sampling its process tree every 200 ms. Input: the test binary. Returns the run's numbers as an object.
function Run-One($exe) {
  $out = Join-Path $env:TEMP "download-bench.out"; Remove-Item $out -ErrorAction SilentlyContinue
  $env:CUE_BENCH_URL = $Url; $env:CUE_BENCH_SHA256 = $Sha256; $env:CUE_BENCH_SIZE = "$Size"; $env:CUE_BENCH_DIR = $Dir
  $p = Start-Process $exe -ArgumentList "download_bench", "--ignored", "--nocapture", "--test-threads=1" -PassThru -NoNewWindow -RedirectStandardOutput $out
  $peak = 0L; $startThreads = $null; $idleThreads = $null; $cpuStart = $null; $cpuIdle = $null; $cpuEnd = $null; $idleAt = $null
  while (-not $p.HasExited) {
    $lines = Bench-Lines $out; $s = Snapshot $p.Id
    $peak = [math]::Max($peak, $s.PrivateBytes)
    if ($null -eq $startThreads -and ($lines -contains "BENCH start")) { $startThreads = $s.Threads }
    if ($null -eq $idleAt -and ($lines -contains "BENCH idle-start")) { $idleAt = Get-Date; $cpuIdle = $s.Cpu }
    if ($idleAt) { $idleThreads = $s.Threads; $cpuEnd = $s.Cpu }
    Start-Sleep -Milliseconds 200
  }
  $kv = @{}; foreach ($l in Bench-Lines $out) { if ($l -match "^BENCH (\w+)=(.*)$") { $kv[$Matches[1]] = $Matches[2] } }
  if ($kv["ok"] -notlike "true*") { throw "run failed: $((Get-Content $out) -join "`n")" }
  $ms = [double]($kv["fetch_ms"])
  [pscustomobject]@{
    FetchMs = $ms; MBps = [math]::Round($Size / 1MB / ($ms / 1000), 1); FirstByte = $kv["first_byte_ms"]; Tail = ($kv["tail_ms"] -split " ")[0]
    VerifyMs = ($kv["verify_ms"] -split " ")[0]; Retries = $kv["retries"]; Wire = $kv["wire_bytes"]
    PeakMB = [math]::Round($peak / 1MB, 1); StartThreads = $startThreads; IdleThreads = $idleThreads
    IdleCpuS = if ($null -ne $cpuIdle) { [math]::Round($cpuEnd - $cpuIdle, 3) } else { $null }
  }
}

# Returns the median of a list of numbers.
function Median($v) { $s = @($v | Sort-Object); $s[[int][math]::Floor(($s.Count - 1) / 2)] }

$exe = Build-Bench
$rows = 1..$Runs | ForEach-Object { $r = Run-One $exe; "run $_ : $($r | ConvertTo-Json -Compress)"; $r }
"median fetch {0} ms, {1} MB/s, peak {2} MB" -f (Median $rows.FetchMs), (Median $rows.MBps), (Median $rows.PeakMB)
"idle: threads start/idle = $(($rows | ForEach-Object { "$($_.StartThreads)/$($_.IdleThreads)" }) -join ' '), idle CPU s = $(($rows.IdleCpuS) -join ' ')"
$log = Join-Path $env:TEMP "download-bench-log.csv"
$rows | Select-Object @{ n = "Label"; e = { $Label } }, @{ n = "Size"; e = { $Size } }, * | Export-Csv $log -Append -NoTypeInformation
if ($Aria2) {
  $times = 1..$Runs | ForEach-Object {
    Remove-Item (Join-Path $Dir "aria2.bin*") -ErrorAction SilentlyContinue
    (Measure-Command { & $Aria2 -x1 -s1 -q -d $Dir -o aria2.bin $Url }).TotalMilliseconds
  }
  "aria2c (1 connection) median {0} ms" -f (Median $times)
}
