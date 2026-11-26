# File: scripts/lib/notepad.ps1
# Purpose: start Notepad for a harness only when no Notepad is running, and close only the processes it started while they show nothing but the harness's own file, so a check never touches the user's own documents.
# Contents: Get-NotepadNames, Get-OwnedNotepad, Start-OwnedNotepad, Stop-OwnedNotepad.
# Usage: . "$PSScriptRoot\..\lib\notepad.ps1" after wait.ps1 and win32.ps1 are loaded. Ownership rule (Windows 11 Notepad is packaged): the process Start-Process returns is a launcher that stays alive as the parent of the Notepad.exe that shows the window, and a later launch by the user opens its file as a tab in that same window process. So the harness owns the launcher and its children, and only while every window title and tab name holds its unique file name.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes

# Returns the titles of a process's visible windows and the names of the tabs inside them. Input: process id.
function Get-NotepadNames($id) {
  $names = @([Win32]::Titles([uint32]$id))
  $own = New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ProcessIdProperty, [int]$id)
  $tab = New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::TabItem)
  foreach ($w in [Windows.Automation.AutomationElement]::RootElement.FindAll("Children", $own)) { $names += @($w.FindAll("Descendants", $tab) | ForEach-Object { $_.Current.Name }) }
  $names
}

# Returns the launcher id followed by its children's ids once a child shows a window and every window title and tab name of every child contains `$titlePart`; $null otherwise. Inputs: launcher process id, title text.
function Get-OwnedNotepad($launcher, $titlePart) {
  $kids = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$launcher" | ForEach-Object { [int]$_.ProcessId })
  $names = @($kids | ForEach-Object { Get-NotepadNames $_ })
  if (-not $names -or ($names | Where-Object { $_ -notlike "*$titlePart*" })) { return $null }
  @([int]$launcher) + $kids
}

# Starts Notepad on `$file` unless a process of that name already runs, and waits up to 20 s until it shows only `$titlePart`. Inputs: file, the unique title text (its file name), process name (the self-check passes its own). Returns the owned ids (see Get-OwnedNotepad), or $null without starting anything when a live one was already running; throws, leaving it open, when the window shows anything else.
function Start-OwnedNotepad($file, $titlePart, $name = "notepad") {
  # an exited process stays listed while this session still holds a handle to it (UI Automation keeps one for seconds), so only a live one counts
  if (Get-Process $name -ErrorAction SilentlyContinue | Where-Object { try { -not $_.HasExited } catch { $true } }) { return $null }
  $launcher = Start-Process $name -ArgumentList "`"$file`"" -PassThru
  $id = $launcher.Id; $launcher.Dispose()
  $null = Await "Notepad to show only $titlePart" { $null -ne (Get-OwnedNotepad $id $titlePart) } 20000
  Get-OwnedNotepad $id $titlePart
}

# Stops the given process ids and waits up to 10 s for each to exit, after checking that every window title and tab name they still show contains `$titlePart`. Throws, stopping nothing, when one shows anything else (a document the user opened into it). Inputs: the ids Start-OwnedNotepad returned ($null stops nothing), the same title text.
function Stop-OwnedNotepad($ids, $titlePart) {
  $foreign = @(foreach ($id in $ids) { Get-NotepadNames $id }) | Where-Object { $_ -notlike "*$titlePart*" }
  if ($foreign) { throw "Notepad now also shows $($foreign -join ', '), so it is left open; close it yourself." }
  foreach ($id in $ids) {
    Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
    Wait-Process -Id $id -Timeout 10 -ErrorAction SilentlyContinue
  }
}
