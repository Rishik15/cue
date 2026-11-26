# File: scripts/lib/notepad.ps1
# Purpose: start Notepad for a harness only when no Notepad is running, and close only the processes it started, so a check never touches the user's own documents.
# Contents: Start-OwnedNotepad, Stop-OwnedNotepad.
# Usage: . "$PSScriptRoot\..\lib\notepad.ps1" after wait.ps1 is loaded.

# Starts Notepad (on `$file` when given) unless a process of that name already runs, and waits up to 20 s for a window whose title contains `$titlePart`. Inputs: file or $null, title text, process name (the self-check passes its own). Returns the ids of the processes of that name once the window shows, or $null without starting anything when one was already running.
function Start-OwnedNotepad($file, $titlePart, $name = "notepad") {
  if (Get-Process $name -ErrorAction SilentlyContinue) { return $null }
  if ($file) { Start-Process $name -ArgumentList "`"$file`"" } else { Start-Process $name }
  $null = Await "Notepad to show $titlePart" { @(Get-Process $name -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -like "*$titlePart*" }).Count -gt 0 } 20000
  @(Get-Process $name -ErrorAction SilentlyContinue | ForEach-Object Id)
}

# Stops exactly the given process ids and waits up to 10 s for each to exit. Input: the ids Start-OwnedNotepad returned ($null stops nothing).
function Stop-OwnedNotepad($ids) {
  foreach ($id in $ids) {
    Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
    Wait-Process -Id $id -Timeout 10 -ErrorAction SilentlyContinue
  }
}
