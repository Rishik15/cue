# File: scripts/lib/misses.ps1
# Purpose: read what one press of the Transform Text shortcut did from the cue.log text written since the press, so a repeat harness can count misses by reason.
# Contents: $selectionEnds (the lines that end a press), Test-PressEnded, Get-PressOutcome.

# Log text that ends a press: the strip shown, a gate, a capture result or a notice. Anything else is still in progress.
$selectionEnds = "strip: visible", "selection: press ignored", "selection: the focus", "not provably", "selection: none in", "selection: Nothing selected", "selection: Cue could not", "selection: No action", "selection: Could not find", "selection: Selection is too long", "selection: stopped", "selection: cancelled", "problem:"

# Returns whether the log text since a press holds a line that ends the press. Input: the log text.
function Test-PressEnded($log) { [bool]($selectionEnds | Where-Object { $log.Contains($_) }) }

# Returns the outcome of one press from the log text since it: "copied", "none (no update)", "refused proof", "focus moved", "empty (field said nothing selected)", "press ignored: <reason>", "never reached on_press", "problem: <text>" or "other". Input: the log text.
function Get-PressOutcome($log) {
  if (-not $log.Contains("selection: press")) { return "never reached on_press" }
  $ignored = [regex]::Match($log, "selection: press ignored: ([^\r\n]+)")
  if ($ignored.Success) { return "press ignored: " + $ignored.Groups[1].Value }
  if ($log.Contains("selection: the focus")) { return "focus moved" }
  if ($log.Contains("not provably")) { return "refused proof" }
  if ($log.Contains("selection: no clipboard update")) { return "none (no update)" }
  if ($log.Contains("selection: copied") -and $log.Contains("strip: visible")) { return "copied" }
  if ($log.Contains("selection: field Empty") -and $log.Contains("selection: Nothing selected")) { return "empty (field said nothing selected)" }
  $problem = [regex]::Match($log, "problem: ([^\r\n]+)")
  if ($problem.Success) { return "problem: " + $problem.Groups[1].Value }
  "other"
}
