# File: scripts/lib/notepad.ps1
# Purpose: start Notepad for a harness only when no Notepad is running, and close only the harness's own document through Notepad's own close path, never ending a Notepad process, so a check never touches the user's own documents.
# Contents: Get-NotepadWindows, Get-NotepadTabs, Get-NotepadNames, Test-OwnName, Get-OwnedNotepad, Start-OwnedNotepad, Close-OwnedTab, Confirm-DontSave, Stop-OwnedNotepad.
# Usage: . "$PSScriptRoot\..\lib\notepad.ps1" after wait.ps1 and win32.ps1 are loaded. Ownership rule (Windows 11 Notepad 11.2609 is packaged): the process Start-Process returns is a launcher that stays alive as the parent of the Notepad.exe that shows the window, and a later launch by the user opens its file as a tab in that same window process. So each run opens a file with a fresh name of its own (a GUID), owns the launcher and its children while every window title and tab name is exactly that file's, and at the end closes only the tab with exactly its file's name, never one whose name merely contains it: Notepad then ends itself when no tab is left, and keeps (or asks about) anything the user opened into it.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$script:uia = [Windows.Automation.AutomationElement]

# Returns the UI Automation elements of a process's top-level windows. Input: process id.
function Get-NotepadWindows($id) {
  $script:uia::RootElement.FindAll("Children", (New-Object Windows.Automation.PropertyCondition($script:uia::ProcessIdProperty, [int]$id)))
}

# Returns the tab elements inside a window. Input: a window element.
function Get-NotepadTabs($window) {
  @($window.FindAll("Descendants", (New-Object Windows.Automation.PropertyCondition($script:uia::ControlTypeProperty, [Windows.Automation.ControlType]::TabItem))))
}

# Returns the titles of a process's visible windows and the names of the tabs inside them. Input: process id.
function Get-NotepadNames($id) {
  $names = @([Win32]::Titles([uint32]$id))
  foreach ($w in Get-NotepadWindows $id) { $names += @(Get-NotepadTabs $w | ForEach-Object { $_.Current.Name }) }
  $names
}

# Returns whether a window title or tab name is exactly the one Notepad shows for the file (window "<name> - Notepad", or "*<name> - Notepad" while modified; tab "<name>. Unmodified." or "<name>. Modified."). Inputs: the title or tab name, the file name without folder.
function Test-OwnName($name, $leaf) {
  @("$leaf - Notepad", "*$leaf - Notepad", "$leaf. Unmodified.", "$leaf. Modified.") -ccontains $name
}

# Returns the launcher id followed by its children's ids once a child shows a window and every window title and tab name of every child is exactly the file's (see Test-OwnName); $null otherwise. Inputs: launcher process id, the file's full path.
function Get-OwnedNotepad($launcher, $file) {
  $leaf = [IO.Path]::GetFileName($file)
  $kids = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$launcher" | ForEach-Object { [int]$_.ProcessId })
  $names = @($kids | ForEach-Object { Get-NotepadNames $_ })
  if (-not $names -or ($names | Where-Object { -not (Test-OwnName $_ $leaf) })) { return $null }
  @([int]$launcher) + $kids
}

# Starts Notepad on `$file` unless a process of that name already runs, and waits up to 20 s until it shows only that file. Inputs: the file's full path (a fresh per-run name), process name (the self-check passes its stand-in's). Returns the owned ids (see Get-OwnedNotepad), or $null without starting anything when a live one was already running; throws, leaving it open, when the window shows anything else.
function Start-OwnedNotepad($file, $name = "notepad") {
  # an exited process stays listed while this session still holds a handle to it (UI Automation keeps one for seconds), so only a live one counts
  if (Get-Process $name -ErrorAction SilentlyContinue | Where-Object { try { -not $_.HasExited } catch { $true } }) { return $null }
  $launcher = Start-Process $name -ArgumentList "`"$file`"" -PassThru
  $id = $launcher.Id; $launcher.Dispose()
  $null = Await "Notepad to show only $file" { $null -ne (Get-OwnedNotepad $id $file) } 20000
  Get-OwnedNotepad $id $file
}

# Closes one of the harness's tabs the way a person would: it selects the tab and presses that tab's own Close Tab button, which Notepad shows only on the selected tab while it is unmodified. A modified tab has none, so its window is closed instead, but only while that tab is the window's only one (a tab arriving in that instant is closed by Notepad itself, which asks about it when it holds changes). Inputs: the window element, the tab element, the file name for the error. Throws, closing nothing, when the modified tab shares the window with another tab.
function Close-OwnedTab($window, $tab, $leaf) {
  $tab.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select()
  $byId = New-Object Windows.Automation.PropertyCondition($script:uia::AutomationIdProperty, "CloseButton")
  $script:closeButton = $null
  $null = WaitUntil { ($script:closeButton = $tab.FindFirst("Children", $byId)) } 1000
  if ($script:closeButton) { return $script:closeButton.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke() }
  if ((Get-NotepadTabs $window).Count -ne 1) { throw "Notepad shows another document beside the check's unsaved $leaf, so it is left open; close that tab yourself (Don't save)." }
  $window.GetCurrentPattern([Windows.Automation.WindowPattern]::Pattern).Close()
}

# Answers Notepad's save prompt with "Don't save", but only a prompt whose text is exactly "Do you want to save changes to <the file's full path>?"; a prompt about any other document, or in other words, is left for the user. Inputs: process id, the file's full path.
function Confirm-DontSave($id, $file) {
  $button = New-Object Windows.Automation.PropertyCondition($script:uia::AutomationIdProperty, "SecondaryButton")
  foreach ($w in Get-NotepadWindows $id) {
    foreach ($b in $w.FindAll("Descendants", $button)) {
      $dialog = [Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($b)
      $texts = $dialog.FindAll("Descendants", (New-Object Windows.Automation.PropertyCondition($script:uia::ControlTypeProperty, [Windows.Automation.ControlType]::Text)))
      if (@($texts | Where-Object { $_.Current.Name -ceq "Do you want to save changes to ${file}?" }).Count) { $b.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke() }
    }
  }
}

# Closes the harness's own tab (the one named exactly for its file) in the given processes and waits up to 10 s until no window or tab is named for the file, answering the save prompt that names the file's full path with "Don't save"; then waits up to 5 s for the processes to end by themselves. Never ends a process: one that still holds a document the user opened stays running. Inputs: the ids Start-OwnedNotepad returned ($null closes nothing), the same file path. Throws, closing nothing, when more than one tab in a window carries the file's name (it cannot tell which is its own), when its tab cannot be closed (see Close-OwnedTab), or when it is still shown after 10 s.
function Stop-OwnedNotepad($ids, $file) {
  $leaf = [IO.Path]::GetFileName($file)
  foreach ($id in $ids) {
    foreach ($w in Get-NotepadWindows $id) {
      $own = @(Get-NotepadTabs $w | Where-Object { Test-OwnName $_.Current.Name $leaf })
      if ($own.Count -gt 1) { throw "Notepad shows more than one tab named $leaf, so none is closed; close the check's one yourself (Don't save)." }
      foreach ($t in $own) { Close-OwnedTab $w $t $leaf }
    }
  }
  $null = Await "Notepad to close $leaf" { foreach ($id in $ids) { Confirm-DontSave $id $file }; -not (@($ids | ForEach-Object { Get-NotepadNames $_ }) | Where-Object { Test-OwnName $_ $leaf }) } 10000
  $null = WaitUntil { -not ($ids | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue | Where-Object { try { -not $_.HasExited } catch { $true } } }) } 5000
}
