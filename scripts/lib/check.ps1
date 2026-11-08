# File: scripts/lib/check.ps1
# Purpose: one pass/fail recorder for the harnesses, so every check prints and counts the same way.
# Contents: the $checks table, Check (record and print one outcome), Complete-Checks (print PASS or FAIL and exit 1 on any failure).
$script:checks = [ordered]@{}

# Records and prints one check. Inputs: name, boolean outcome.
function Check($name, $ok) { $script:checks[$name] = [bool]$ok; "{0,-58} {1}" -f $name, $(if ($ok) { "ok" } else { "FAILED" }) }

# Prints PASS, or FAIL and exits 1 when any recorded check failed.
function Complete-Checks { if ($script:checks.Values -contains $false) { "FAIL"; exit 1 } else { "PASS" } }
