# File: scripts/test/clipguard.check.ps1
# Purpose: self-check that every harness that writes the clipboard can give the user's clipboard back: it loads scripts/lib/clipformats.ps1, refuses a clipboard holding more than plain text (Assert-TextClipboard) on the line before it saves the text into $clipboard, puts $clipboard back in a finally block, first or through `Protected`, so no cleanup step that throws can skip it, and writes nothing to the clipboard before that save (no top-level statement before it writes, or calls a function of the script that writes, directly or through another). A new harness that writes the clipboard without this fails here.
# Contents: Restores (the parsed restore statements), WritesBeforeSave (clipboard writes run before the save), the scan of scripts/verify and scripts/lab, the per-harness checks, the verdict.
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

# Returns whether the script can write the clipboard before it saves the user's text: a top-level statement before the `$clipboard = ...GetText()` save that writes (a Clipboard Set* or Clear call, or Set-Clipboard) or calls a function of the script that writes, directly or through another. Also true when there is no top-level save. Input: script path. Returns the boolean.
function WritesBeforeSave($path) {
  $ast = [System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$null, [ref]$null)
  $writes = { param($n) ($n -is [System.Management.Automation.Language.InvokeMemberExpressionAst] -and $n.Expression.Extent.Text -match 'Clipboard\]$' -and $n.Member.Value -match '^(Set|Clear)') -or ($n -is [System.Management.Automation.Language.CommandAst] -and ($n.GetCommandName() -eq "Set-Clipboard" -or $writerNames -contains $n.GetCommandName())) }
  $functions = @($ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true))
  $writerNames = @()
  do {
    $before = $writerNames.Count
    $writerNames = @($functions | Where-Object { $_.Body.Find($writes, $true) } | ForEach-Object Name)
  } while ($writerNames.Count -ne $before)
  $top = @($ast.EndBlock.Statements)
  $save = @($top | ForEach-Object { $_.Extent.Text }).IndexOf('$clipboard = [System.Windows.Forms.Clipboard]::GetText()')
  if ($save -lt 0) { return $true }
  [bool]@($top[0..$save] | Where-Object { $_ -isnot [System.Management.Automation.Language.FunctionDefinitionAst] -and $_.Find($writes, $true) }).Count
}

$writers = @(Get-ChildItem (Join-Path $PSScriptRoot "..\verify"), (Join-Path $PSScriptRoot "..\lab") -Filter *.ps1 | Where-Object { (Get-Content -LiteralPath $_.FullName -Raw) -match 'Clipboard\]::Set|Set-Clipboard' })
Check "the scan finds the seven clipboard-writing harnesses" ($writers.Count -ge 7)
foreach ($f in $writers) {
  $text = Get-Content -LiteralPath $f.FullName -Raw
  Check "$($f.Name): loads clipformats.ps1" ($text -match 'lib\\clipformats\.ps1')
  Check "$($f.Name): refuses non-text, then saves the text" ($text -match '(?m)^Assert-TextClipboard\r?\n\$clipboard = \[System\.Windows\.Forms\.Clipboard\]::GetText\(\)')
  $r = @(Restores $f.FullName)
  Check "$($f.Name): writes nothing to the clipboard before the save" (-not (WritesBeforeSave $f.FullName))
  Check "$($f.Name): puts the saved text back first in a finally" ($r.Count -ge 1 -and -not ($r -contains $false))
}
Complete-Checks
