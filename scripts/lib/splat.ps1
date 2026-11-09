# File: scripts/lib/splat.ps1
# Purpose: turn command-line words like `-Cycles 20 -Show` into a hashtable that can be splatted onto a script, because splatting an array passes `-Gate` as plain text instead of a parameter name.
# Contents: ConvertTo-Splat — words in, hashtable out (a word that starts with `-` is a name; the next word is its value unless it also starts with `-`).

# Builds a parameter hashtable from `words`. Throws on a value without a name.
function ConvertTo-Splat([string[]]$words) {
  $splat = @{}
  for ($i = 0; $i -lt $words.Count; $i++) {
    if ($words[$i] -notmatch "^-(\w+)$") { throw "Unexpected argument '$($words[$i])'." }
    $name = $Matches[1]
    $hasValue = ($i + 1 -lt $words.Count) -and ($words[$i + 1] -notmatch "^-[A-Za-z]")
    $splat[$name] = if ($hasValue) { $words[++$i] } else { $true }
  }
  $splat
}
