# File: scripts/lib/notepad.ps1
# Purpose: start Notepad for a harness only when no Notepad is running, and close only the harness's own document through Notepad's own close path, never ending a Notepad process, so a check never touches the user's own documents.
# Contents: Get-NotepadWindows, Get-NotepadTabs, Get-NotepadNames, Get-OwnedNotepad, Start-OwnedNotepad, Close-OwnedTab, Confirm-DontSave, Stop-OwnedNotepad.
# Usage: . "$PSScriptRoot\..\lib\notepad.ps1" after wait.ps1 and win32.ps1 are loaded. Ownership rule (Windows 11 Notepad 11.2609 is packaged): the process Start-Process returns is a launcher that stays alive as the parent of the Notepad.exe that shows the window, and a later launch by the user opens its file as a tab in that same window process. So the harness owns the launcher and its children while every window title and tab name holds its unique file name, and at the end it closes only its own tab: Notepad then ends itself when no tab is left, and keeps (or asks about) anything the user opened into it.
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

# Returns the launcher id followed by its children's ids once a child shows a window and every window title and tab name of every child contains `$titlePart`; $null otherwise. Inputs: launcher process id, title text.
function Get-OwnedNotepad($launcher, $titlePart) {
  $kids = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$launcher" | ForEach-Object { [int]$_.ProcessId })
  $names = @($kids | ForEach-Object { Get-NotepadNames $_ })
  if (-not $names -or ($names | Where-Object { $_ -notlike "*$titlePart*" })) { return $null }
  @([int]$launcher) + $kids
}

# Starts Notepad on `$file` unless a process of that name already runs, and waits up to 20 s until it shows only `$titlePart`. Inputs: file, the unique title text (its file name), process name (the self-check passes its stand-in's). Returns the owned ids (see Get-OwnedNotepad), or $null without starting anything when a live one was already running; throws, leaving it open, when the window shows anything else.
function Start-OwnedNotepad($file, $titlePart, $name = "notepad") {
  # an exited process stays listed while this session still holds a handle to it (UI Automation keeps one for seconds), so only a live one counts
  if (Get-Process $name -ErrorAction SilentlyContinue | Where-Object { try { -not $_.HasExited } catch { $true } }) { return $null }
  $launcher = Start-Process $name -ArgumentList "`"$file`"" -PassThru
  $id = $launcher.Id; $launcher.Dispose()
  $null = Await "Notepad to show only $titlePart" { $null -ne (Get-OwnedNotepad $id $titlePart) } 20000
  Get-OwnedNotepad $id $titlePart
}

# Closes one of the harness's tabs the way a person would: it selects the tab and presses that tab's own Close Tab button, which Notepad shows only on the selected tab while it is unmodified. A modified tab has none, so its window is closed instead, but only while that tab is the window's only one (a tab arriving in that instant is closed by Notepad itself, which asks about it when it holds changes). Inputs: the window element, the tab element, title text. Throws, closing nothing, when the modified tab shares the window with another tab.
function Close-OwnedTab($window, $tab, $titlePart) {
  $tab.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select()
  $byId = New-Object Windows.Automation.PropertyCondition($script:uia::AutomationIdProperty, "CloseButton")
  $script:closeButton = $null
  $null = WaitUntil { ($script:closeButton = $tab.FindFirst("Children", $byId)) } 1000
  if ($script:closeButton) { return $script:closeButton.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke() }
  if ((Get-NotepadTabs $window).Count -ne 1) { throw "Notepad shows another document beside the check's unsaved $titlePart, so it is left open; close that tab yourself (Don't save)." }
  $window.GetCurrentPattern([Windows.Automation.WindowPattern]::Pattern).Close()
}

# Answers Notepad's save prompt with "Don't save", but only a prompt whose text names `$titlePart`; a prompt about any other document is left for the user. Inputs: process id, title text.
function Confirm-DontSave($id, $titlePart) {
  $button = New-Object Windows.Automation.PropertyCondition($script:uia::AutomationIdProperty, "SecondaryButton")
  foreach ($w in Get-NotepadWindows $id) {
    foreach ($b in $w.FindAll("Descendants", $button)) {
      $dialog = [Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($b)
      $texts = $dialog.FindAll("Descendants", (New-Object Windows.Automation.PropertyCondition($script:uia::ControlTypeProperty, [Windows.Automation.ControlType]::Text)))
      if (@($texts | Where-Object { $_.Current.Name -like "*$titlePart*" }).Count) { $b.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke() }
    }
  }
}

# Closes the harness's own tabs in the given processes and waits up to 10 s until no window or tab shows `$titlePart`, answering a save prompt about it with "Don't save"; then waits up to 5 s for the processes to end by themselves. Never ends a process: one that still holds a document the user opened stays running. Inputs: the ids Start-OwnedNotepad returned ($null closes nothing), the same title text. Throws when its tab cannot be closed (see Close-OwnedTab) or is still shown after 10 s.
function Stop-OwnedNotepad($ids, $titlePart) {
  foreach ($id in $ids) {
    foreach ($w in Get-NotepadWindows $id) {
      foreach ($t in Get-NotepadTabs $w) { if ($t.Current.Name -like "*$titlePart*") { Close-OwnedTab $w $t $titlePart } }
    }
  }
  $null = Await "Notepad to close $titlePart" { foreach ($id in $ids) { Confirm-DontSave $id $titlePart }; -not (@($ids | ForEach-Object { Get-NotepadNames $_ }) -like "*$titlePart*") } 10000
  $null = WaitUntil { -not ($ids | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue | Where-Object { try { -not $_.HasExited } catch { $true } } }) } 5000
}
