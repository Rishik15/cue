# File: scripts/verify/bench-report.ps1
# Purpose: show a bench result next to bench/baseline.json as a Markdown table, without judging it. Performance is tracked, not gated: a person reads the table. Every metric either side holds gets a row, and numbers accumulated over a window or a cycle count are compared only on equal footing.
# Contents: $info (recipe and machine fields kept out of the table), Format-Cell, Format-Change (one change cell), Get-Rate (CPU seconds per second of a window), Format-Row (one metric, with the CPU rate and the cycle check), the table, the machine and recipe note, the optional step-summary output.
# Usage: pwsh scripts/verify/bench-report.ps1 [-Result bench/latest.json] [-Baseline bench/baseline.json] [-Summary path]   (-Summary appends the table to a file, for $env:GITHUB_STEP_SUMMARY)
param(
  [string]$Result = (Join-Path $PSScriptRoot "..\..\bench\latest.json"),
  [string]$Baseline = (Join-Path $PSScriptRoot "..\..\bench\baseline.json"),
  [string]$Summary = ""
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Result)) { throw "No result to report: $Result. Run the bench first." }
$now = Get-Content $Result -Raw | ConvertFrom-Json
$base = Get-Content $Baseline -Raw | ConvertFrom-Json
$info = "schema", "machine", "os_build", "webview2", "cores", "idle_window_seconds", "goal", "commit", "cycles", "dictations"
$rates = "idle_cpu_seconds", "after_close_cpu_seconds"

# Returns the text of one value cell: "skipped" for a missing value, a list joined with commas. Input: the value.
function Format-Cell($v) { if ($null -eq $v) { "skipped" } else { @($v) -join ", " } }

# Returns the change cell for one metric: a percentage, or n/a plus the reason when a side is a list or the baseline is zero, missing or from another schema. Inputs: current value, baseline value.
function Format-Change($cur, $ref) {
  if ($null -eq $cur) { return "skipped" }
  if ($null -eq $ref -or $ref -eq 0) { return "n/a (baseline $(if ($null -eq $ref) { 'missing' } else { '0' }))" }
  if ($cur -is [array] -or $ref -is [array]) { return "n/a (list)" }
  "{0:+0.#;-0.#;0}%" -f (($cur - $ref) / $ref * 100)
}

# Returns CPU seconds per second of the window they were measured over, to 4 decimals, or $null when either is missing. Inputs: CPU seconds, window seconds.
function Get-Rate($cpu, $window) { if ($null -eq $cpu -or -not $window) { return $null }; [math]::Round($cpu / $window, 4) }

# Returns the table row of one metric. CPU seconds become a rate over each side's own idle window; the leak, accumulated over the open/close cycles, is n/a when the cycle counts differ or either side has none. Input: the metric name.
function Format-Row($name) {
  $cur = $now.$name; $ref = $base.$name; $label = $name
  if ($rates -contains $name) { $cur = Get-Rate $cur $now.idle_window_seconds; $ref = Get-Rate $ref $base.idle_window_seconds; $label = "$name (per s)" }
  $change = Format-Change $cur $ref
  if ($name -eq "leak_delta_mb" -and $null -ne $cur -and ($null -eq $now.cycles -or $now.cycles -ne $base.cycles)) {
    $counts = $now.cycles, $base.cycles | ForEach-Object { if ($null -eq $_) { "none" } else { $_ } }
    $change = "n/a (cycles $($counts[0]) vs $($counts[1]))"
  }
  "| $label | $(Format-Cell $cur) | $(if ($null -eq $ref) { '' } else { Format-Cell $ref }) | $change |"
}

$names = @($now.PSObject.Properties | ForEach-Object Name) + @($base.PSObject.Properties | ForEach-Object Name) | Where-Object { $info -notcontains $_ } | Select-Object -Unique
$rows = @("| metric | now | baseline | change |", "| --- | ---: | ---: | ---: |") + @($names | ForEach-Object { Format-Row $_ })
$note = "Measured on $($now.machine), Windows $($now.os_build), WebView2 $($now.webview2), $($now.cores) cores, commit $($now.commit), $($now.cycles) cycles, $($now.dictations) dictations. Baseline from $($base.machine), Windows $($base.os_build)$(if ($null -ne $base.cycles) { ", $($base.cycles) cycles" }). A table from a different kind of machine shows direction only."
$note += " CPU rows are CPU seconds per second of idle (window now $($now.idle_window_seconds) s, baseline $($base.idle_window_seconds) s)."
if ("github-runner" -in $now.machine, $base.machine) { $note += " Numbers from a shared runner (machine: github-runner) are advisory; compare only with a run on the same machine kind." }
if ($base.goal) { $note += " Goal: $($base.goal)." }
if ($now.schema -ne $base.schema) { $note += " Schemas differ ($($now.schema) vs $($base.schema)): compare by eye." }
$text = ($rows + "" + $note) -join "`n"
$text
if ($Summary) { Add-Content -Path $Summary -Value "### Bench against bench/baseline.json`n`n$text" }
