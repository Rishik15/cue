# File: scripts/test/notepad.check.ps1
# Purpose: self-check of scripts/lib/notepad.ps1 on real processes and real UI Automation tabs: Start-OwnedNotepad starts nothing while a process of that name runs; started on a file it owns the launcher and its window child; Stop-OwnedNotepad closes the owned tab and the processes then end by themselves; tabs that arrive later (inactive, under the owned window title, one unmodified and one modified, both named with the check's file name inside theirs) end ownership, and Stop-OwnedNotepad then closes only the check's tab, answers only the save prompt naming the check's exact file (not one whose folder holds its name), and leaves those tabs and both processes running.
# Contents: $standIn (the stand-in's C# source), Get-Control, Get-Texts, the three cases, the verdict.
# Usage: & scripts/test/notepad.check.ps1   (cue.ps1 test runs it; it never starts or stops a Notepad: its stand-in is a small WPF exe it compiles, which mirrors Notepad 11's launcher, its tab names, Close Tab buttons and save prompt)
$ErrorActionPreference = "Stop"
$saved = @{ APPDATA = $env:APPDATA; TEMP = $env:TEMP; PATH = $env:PATH }
$root = Join-Path ([IO.Path]::GetTempPath()) ("cue-notepad-check-" + [guid]::NewGuid())
. (Join-Path $PSScriptRoot "..\lib\check.ps1")
. (Join-Path $PSScriptRoot "..\lib\wait.ps1")
$name = "cue-notepad-stand-in"
$standIn = @'
using System; using System.Diagnostics; using System.IO; using System.Windows; using System.Windows.Automation; using System.Windows.Controls;
/// A Notepad stand-in for scripts/test/notepad.check.ps1.
public static class StandIn {
  /// Started on a file, it starts itself again with --window and that file and waits for that child, as Notepad 11's launcher does. With --window it shows a minimised window titled "<selected tab's file> - Notepad", with the file's tab, an Add New Tab button that adds two inactive tabs whose names hold the file's name ("<name>-notes.txt" unmodified, "<name>-draft.txt" modified) and a save prompt for "notes.txt" in a folder "<name>-dir", and an Ask button that shows a save prompt for the file itself. Input: the command-line arguments.
  [STAThread] public static void Main(string[] args) {
    if (args[0] != "--window") { Process.Start(Process.GetCurrentProcess().MainModule.FileName, "--window \"" + args[0] + "\"").WaitForExit(); return; }
    string file = Path.GetFileName(args[1]), stem = Path.GetFileNameWithoutExtension(args[1]);
    var tabs = new TabControl();
    var window = new Window { ShowActivated = false, ShowInTaskbar = false, WindowState = WindowState.Minimized, Width = 400, Height = 300 };
    var panel = new DockPanel();
    var add = new Button { Content = "+" }; AutomationProperties.SetAutomationId(add, "AddButton");
    add.Click += (s, e) => { tabs.Items.Add(Tab(tabs, window, stem + "-notes.txt", false)); tabs.Items.Add(Tab(tabs, window, stem + "-draft.txt", true)); Prompt(panel, Path.Combine(Path.GetDirectoryName(args[1]), stem + "-dir", "notes.txt")); };
    var ask = new Button { Content = "?" }; AutomationProperties.SetAutomationId(ask, "AskButton");
    ask.Click += (s, e) => Prompt(panel, args[1]);
    DockPanel.SetDock(add, Dock.Top); DockPanel.SetDock(ask, Dock.Top); panel.Children.Add(add); panel.Children.Add(ask); panel.Children.Add(tabs);
    window.Content = panel;
    tabs.SelectionChanged += (s, e) => { if (tabs.SelectedItem != null) window.Title = ((TabItem)tabs.SelectedItem).Tag + " - Notepad"; };
    tabs.Items.Add(Tab(tabs, window, file, false)); tabs.SelectedIndex = 0;
    new Application().Run(window);
  }
  /// Returns a tab named as Notepad names it ("<file>. Unmodified." or "<file>. Modified."). An unmodified tab's header is its Close Tab button, which removes the tab and closes the window with the last one, ending the process; a modified tab has no button, as in Notepad. Inputs: the tab control, the window, the file name, whether it is modified.
  static TabItem Tab(TabControl tabs, Window window, string file, bool modified) {
    var tab = new TabItem { Tag = file }; AutomationProperties.SetName(tab, file + (modified ? ". Modified." : ". Unmodified."));
    if (modified) { tab.Header = "*"; return tab; }
    var close = new Button { Content = "x" }; AutomationProperties.SetAutomationId(close, "CloseButton");
    close.Click += (s, e) => { tabs.Items.Remove(tab); if (tabs.Items.Count == 0) window.Close(); };
    tab.Header = close;
    return tab;
  }
  /// Shows a save prompt as Notepad's: a group holding the text "Do you want to save changes to <path>?" and a Don't save button (aid SecondaryButton) that removes the prompt. Inputs: the panel, the path.
  static void Prompt(DockPanel panel, string path) {
    var dontSave = new Button { Content = "Don't save" }; AutomationProperties.SetAutomationId(dontSave, "SecondaryButton");
    var body = new StackPanel(); body.Children.Add(new TextBlock { Text = "Do you want to save changes to " + path + "?" }); body.Children.Add(dontSave);
    var group = new GroupBox { Content = body }; DockPanel.SetDock(group, Dock.Top);
    dontSave.Click += (s, e) => panel.Children.Remove(group);
    panel.Children.Insert(0, group);
  }
}
'@

# Returns the UI Automation element of a process's window control with the given automation id. Inputs: process id, automation id.
function Get-Control($id, $automationId) {
  $w = Get-NotepadWindows $id | Select-Object -First 1
  $w.FindFirst("Descendants", (New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty, $automationId)))
}

# Returns the names of the text elements in a process's window. Input: process id.
function Get-Texts($id) {
  $w = Get-NotepadWindows $id | Select-Object -First 1
  @($w.FindAll("Descendants", (New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Text))) | ForEach-Object { $_.Current.Name })
}

try {
  New-Item -ItemType Directory -Path $root | Out-Null
  $env:APPDATA = $root; $env:TEMP = $root
  . (Join-Path $PSScriptRoot "..\lib\win32.ps1")
  . (Join-Path $PSScriptRoot "..\lib\notepad.ps1")

  $self = (Get-Process -Id $PID).ProcessName
  $before = @(Get-Process $self | ForEach-Object Id)
  $ids = Start-OwnedNotepad $null $self
  $new = @(Get-Process $self | Where-Object { $before -notcontains $_.Id })
  Check "a running process of that name: returns nothing" ($null -eq $ids)
  Check "a running process of that name: starts nothing" ($new.Count -eq 0)

  $refs = @("PresentationFramework", "PresentationCore", "WindowsBase", "System.Xaml", "System", "UIAutomationProvider", "UIAutomationTypes") | ForEach-Object { [Reflection.Assembly]::LoadWithPartialName($_).Location }
  # compiled without Add-Type, which would load the exe into this session and lock it against the final delete
  $options = New-Object CodeDom.Compiler.CompilerParameters (, [string[]]$refs)
  $options.OutputAssembly = Join-Path $root "$name.exe"; $options.GenerateExecutable = $true; $options.CompilerOptions = "/target:winexe"
  $built = (New-Object Microsoft.CSharp.CSharpCodeProvider).CompileAssemblyFromSource($options, $standIn)
  if ($built.Errors.HasErrors) { throw "The Notepad stand-in did not compile: $($built.Errors -join '; ')" }
  $env:PATH = "$root;$env:PATH"

  $part = "cue-notepad-check-" + [guid]::NewGuid()
  $file = Join-Path $root "$part.txt"; Set-Content $file ""
  $owned = @(Start-OwnedNotepad $file $name)
  $procs = @($owned | ForEach-Object { Get-Process -Id $_ })
  Check "started on its file: owns the launcher and its window child" (($owned.Count -eq 2) -and ((Get-NotepadNames $owned[1]) -contains "$part.txt - Notepad"))
  Stop-OwnedNotepad $owned $file
  Check "Stop-OwnedNotepad closes the owned tab and both processes end by themselves" (-not ($procs | Where-Object { -not $_.HasExited }))

  $part = "cue-notepad-check-" + [guid]::NewGuid()
  $file = Join-Path $root "$part.txt"; Set-Content $file ""
  $owned = @(Start-OwnedNotepad $file $name)
  $procs = @($owned | ForEach-Object { Get-Process -Id $_ })
  foreach ($button in "AddButton", "AskButton") { (Get-Control $owned[1] $button).GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke() }
  $ownPrompt = "Do you want to save changes to ${file}?"; $foreignPrompt = "Do you want to save changes to $(Join-Path $root "$part-dir\notes.txt")?"
  $null = Await "the later tabs and prompts to arrive" { ((Get-NotepadNames $owned[1]) -contains "$part-draft.txt. Modified.") -and ((Get-Texts $owned[1]) -contains $ownPrompt) } 10000
  Check "later inactive tabs under the owned title: the window title is still owned" ([Win32]::Titles([uint32]$owned[1]) -contains "$part.txt - Notepad")
  Check "later inactive tabs holding the file's name: no longer owned" ($null -eq (Get-OwnedNotepad $owned[0] $file))
  Stop-OwnedNotepad $owned $file
  $left = @(Get-NotepadNames $owned[1])
  Check "Stop-OwnedNotepad closes only the check's tab" ((-not ($left | Where-Object { Test-OwnName $_ "$part.txt" })) -and ($left -contains "$part-notes.txt. Unmodified.") -and ($left -contains "$part-draft.txt. Modified."))
  $answered = $null -ne (WaitUntil { -not ((Get-Texts $owned[1]) -contains $ownPrompt) } 2000)
  Check "Stop-OwnedNotepad answers only the prompt naming the check's exact file" ($answered -and ((Get-Texts $owned[1]) -contains $foreignPrompt))
  Check "Stop-OwnedNotepad leaves both processes running" (-not ($procs | Where-Object { $_.HasExited }))
} finally {
  Get-Process $name -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  $env:APPDATA = $saved.APPDATA; $env:TEMP = $saved.TEMP; $env:PATH = $saved.PATH
  # an ended stand-in keeps its exe locked for a moment
  $null = WaitUntil { Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue; -not (Test-Path $root) } 10000
}
Complete-Checks
