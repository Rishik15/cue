# File: scripts/test/clipguard.check.ps1
# Purpose: self-check that every harness that writes the clipboard can give the user's clipboard back: it loads scripts/lib/clipformats.ps1, refuses a clipboard holding more than plain text (Assert-TextClipboard) on the line before it saves the text into $clipboard, and puts $clipboard back in a finally block, first or through `Protected`, so no cleanup step that throws can skip it. A new harness that writes the clipboard without this fails here.
# Contents: Restores (the parsed restore statements), the scan of scripts/verify and scripts/lab, the per-harness checks, the verdict.
# Usage: & scripts/test/clipguard.check.ps1   (cue.ps1 test runs it)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\lib\check.ps1")

# Returns, for each `SetText($clipboard)` call in a script, whether its statement sits in a finally block and is either its first statement or a `Protected` step (which never throws). Input: script path. Returns a list of booleans.
function Restores($path) {
  $ast = [System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$null, [ref]$null)
  $calls = $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.InvokeMemberExpressionAst] -and $n.Member.Value -eq "SetText" -and $n.Arguments.Count -eq 1 -and $n.Arguments[0].Extent.Text -eq '$clipboard' }, $true)
  foreach ($c in $calls) {
    $n = $c; $ok = $false
    while ($n.Parent) {
      $b = $n.Parent
      if ($b -is [System.Management.Automation.Language.StatementBlockAst] -and $b.Parent -is [System.Management.Automation.Language.TryStatementAst] -and $b.Parent.Finally -eq $b) {
        $ok = ($b.Statements[0] -eq $n) -or ($n.Extent.Text -match '^Protected\s'); break
      }
      $n = $b
    }
    $ok
  }
}

$writers = @(Get-ChildItem (Join-Path $PSScriptRoot "..\verify"), (Join-Path $PSScriptRoot "..\lab") -Filter *.ps1 | Where-Object { (Get-Content -LiteralPath $_.FullName -Raw) -match 'Clipboard\]::Set|Set-Clipboard' })
Check "the scan finds the seven clipboard-writing harnesses" ($writers.Count -ge 7)
foreach ($f in $writers) {
  $text = Get-Content -LiteralPath $f.FullName -Raw
  Check "$($f.Name): loads clipformats.ps1" ($text -match 'lib\\clipformats\.ps1')
  Check "$($f.Name): refuses non-text, then saves the text" ($text -match '(?m)^Assert-TextClipboard\r?\n\$clipboard = \[System\.Windows\.Forms\.Clipboard\]::GetText\(\)')
  $r = @(Restores $f.FullName)
  Check "$($f.Name): puts the saved text back first in a finally" ($r.Count -ge 1 -and -not ($r -contains $false))
}
Complete-Checks
