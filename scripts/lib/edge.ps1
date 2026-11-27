# File: scripts/lib/edge.ps1
# Purpose: the command line that opens a local page in a new Edge app window on a profile of its own, quoted so a temp folder with spaces (C:\Users\Jane Doe\...) stays one argument.
# Contents: Get-EdgeArguments (pure).

# Edge's arguments for one app window. Start-Process joins -ArgumentList with spaces into one command line, so each path is quoted here and the page becomes an escaped file URL. Inputs: the profile folder, the page's path, the URL fragment (empty for none). Return: the argument string.
function Get-EdgeArguments($userData, $page, $fragment) {
  $url = ([Uri]$page).AbsoluteUri + $(if ($fragment) { "#$fragment" })
  "`"--user-data-dir=$userData`" --no-first-run --new-window `"--app=$url`""
}
