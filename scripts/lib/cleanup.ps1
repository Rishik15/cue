# File: scripts/lib/cleanup.ps1
# Purpose: one cleanup step runner for the harnesses' finally blocks, so a step that throws cannot skip the restores after it.
# Contents: Protected.

# Runs one cleanup step and reports a failure instead of stopping, so one failed step cannot skip the restores after it. Inputs: what the step does, the step.
function Protected($what, $step) { try { & $step } catch { "WARNING: could not $what`: $_" } }
