# File: scripts/lib/wait.ps1
# Purpose: wait for a state instead of sleeping a fixed time, so a check fails with a named cause when the state never comes.
# Contents: WaitUntil (poll a condition, return the elapsed milliseconds or $null on timeout), Await (the same, but a timeout throws), Wait-Stable (poll a value until two samples agree), Get-LogMark / Read-LogBytes / Read-LogSince (the log since a mark, across one rotation) / Wait-Hotkeys / Wait-HookReady (wait until a launched Cue has installed its keyboard hook; the last returns the hook-ready timestamp).

# Polls a condition every 5 ms. Inputs: condition script block, timeout in ms. Returns the elapsed ms when it held, or $null when the time ran out.
function WaitUntil($cond, $ms) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while (-not (& $cond)) { if ($sw.ElapsedMilliseconds -ge $ms) { return $null }; Start-Sleep -Milliseconds 5 }
  $sw.ElapsedMilliseconds
}

# Like WaitUntil, but a timeout throws "Timed out ... waiting for: <what>". Inputs: description, condition, timeout in ms. Returns the elapsed ms.
function Await($what, $cond, $ms) {
  $t = WaitUntil $cond $ms
  if ($null -eq $t) { throw "Timed out after $($ms / 1000) s waiting for: $what" }
  $t
}

# Samples a value every 400 ms until two samples differ by at most the tolerance, then returns the latest. Inputs: description, sampler script block returning a number, tolerance, timeout in ms. A timeout throws.
function Wait-Stable($what, $sample, $tolerance, $ms) {
  $sw = [Diagnostics.Stopwatch]::StartNew(); $last = $null
  while ($true) {
    $now = & $sample
    if ($null -ne $last -and [math]::Abs($now - $last) -le $tolerance) { return $now }
    if ($sw.ElapsedMilliseconds -ge $ms) { throw "Timed out after $($ms / 1000) s waiting for: $what" }
    $last = $now; Start-Sleep -Milliseconds 400
  }
}

# Returns how long Cue's log is now, to mark where a launch begins. Returns 0 when there is no log yet.
function Get-LogMark {
  $log = Join-Path $env:APPDATA "dev.cue.app\cue.log"
  if (Test-Path $log) { (Get-Item $log).Length } else { 0 }
}

# Returns a log file's bytes, read with sharing so Cue can keep appending (a reader that blocks the writer makes log lines vanish). Input: path. Return: the byte array (not unrolled).
function Read-LogBytes($path) {
  $file = [IO.File]::Open($path, "Open", "Read", "ReadWrite,Delete")
  try { $stream = New-Object IO.MemoryStream; $file.CopyTo($stream); , $stream.ToArray() } finally { $file.Dispose() }
}

# Returns the log text written after `$mark` bytes (see Get-LogMark), cut on bytes so non-ASCII text earlier in the log cannot shift the start. When cue.log is shorter than the mark it has rotated (Cue renames a full log to cue.old.log), so the text is cue.old.log from the mark plus all of the new cue.log. shortcut: a second rotation, or a new log grown past the mark, between mark and read is not detected; a press is far shorter than 256 KB of log.
function Read-LogSince($mark) {
  $dir = Join-Path $env:APPDATA "dev.cue.app"
  $utf8 = [Text.Encoding]::UTF8
  $now = Read-LogBytes (Join-Path $dir "cue.log")
  if ($now.Length -ge $mark) { return $utf8.GetString($now, $mark, $now.Length - $mark) }
  $old = Join-Path $dir "cue.old.log"
  $before = if (Test-Path $old) { $bytes = Read-LogBytes $old; if ($bytes.Length -ge $mark) { $utf8.GetString($bytes, $mark, $bytes.Length - $mark) } }
  "$before" + $utf8.GetString($now)
}

# Waits until the launched Cue has logged that its keyboard hook is ready and returns the monotonic milliseconds that line carries (time since the log began, taken on the input thread); a timeout throws. Input: the log mark from Get-LogMark, timeout in ms.
function Wait-HookReady($mark, $ms = 10000) {
  $log = Join-Path $env:APPDATA "dev.cue.app\cue.log"
  $script:hookReadyMs = $null
  $null = Await "the keyboard hook to be ready" {
    try {
      $m = [regex]::Match((Read-LogSince $mark), "stage: keyboard hook ready at (\d+) ms")
      if ($m.Success) { $script:hookReadyMs = [int]$m.Groups[1].Value }
      $m.Success
    } catch { $false }
  } $ms
  $script:hookReadyMs
}

# Waits until the process started after `$mark` (see Get-LogMark) has installed its keyboard hook: the tray window exists earlier, and a shortcut pressed before the hook is lost. A timeout throws.
function Wait-Hotkeys($mark) {
  $log = Join-Path $env:APPDATA "dev.cue.app\cue.log"
  $null = Await "the keyboard hook to be installed" {
    try {
      (Read-LogSince $mark).Contains("input: keyboard hook installed")
    } catch { $false }
  } 10000
}
