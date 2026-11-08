# File: scripts/lib/wait.ps1
# Purpose: wait for a state instead of sleeping a fixed time, so a check fails with a named cause when the state never comes.
# Contents: WaitUntil (poll a condition, return the elapsed milliseconds or $null on timeout), Await (the same, but a timeout throws), Wait-Stable (poll a value until two samples agree).

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
