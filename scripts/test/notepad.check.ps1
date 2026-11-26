# File: scripts/test/notepad.check.ps1
# Purpose: self-check of scripts/lib/notepad.ps1 on real processes and real UI Automation tabs: Start-OwnedNotepad starts nothing while a process of that name runs; started on a file it owns the launcher and its window child; Stop-OwnedNotepad closes the owned tab and the processes then end by themselves; a tab that arrives later (inactive, under the owned window title) ends ownership, and Stop-OwnedNotepad then closes only the check's tab and leaves that tab and both processes running.
# Contents: $standIn (the stand-in's C# source), Get-Control, the three cases, the verdict.
# Usage: & scripts/test/notepad.check.ps1   (cue.ps1 test runs it; it never starts or stops a Notepad: its stand-in is a small WPF exe it compiles, which mirrors Notepad 11's launcher, its tab names and Close Tab buttons)
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
  /// Started on a file, it starts itself again with --window and that file and waits for that child, as Notepad 11's launcher does. With --window it shows a minimised window titled "<selected tab's file> - Notepad", with the file's tab and an Add New Tab button that adds an inactive tab "my notes.txt". Input: the command-line arguments.
  [STAThread] public static void Main(string[] args) {
    if (args[0] != "--window") { Process.Start(Process.GetCurrentProcess().MainModule.FileName, "--window \"" + args[0] + "\"").WaitForExit(); return; }
    string file = Path.GetFileName(args[1]);
    var tabs = new TabControl();
    var window = new Window { ShowActivated = false, ShowInTaskbar = false, WindowState = WindowState.Minimized, Width = 400, Height = 200 };
    var add = new Button { Content = "+" }; AutomationProperties.SetAutomationId(add, "AddButton");
    add.Click += (s, e) => tabs.Items.Add(Tab(tabs, window, "my notes.txt"));
    var panel = new DockPanel(); DockPanel.SetDock(add, Dock.Top); panel.Children.Add(add); panel.Children.Add(tabs);
    window.Content = panel;
    tabs.SelectionChanged += (s, e) => { if (tabs.SelectedItem != null) window.Title = ((TabItem)tabs.SelectedItem).Tag + " - Notepad"; };
    tabs.Items.Add(Tab(tabs, window, file)); tabs.SelectedIndex = 0;
    new Application().Run(window);
  }
  /// Returns a tab named as Notepad names an unmodified one ("<file>. Unmodified.") whose header is its Close Tab button; the button removes the tab and closes the window with the last one, which ends the process. Inputs: the tab control, the window, the file name.
  static TabItem Tab(TabControl tabs, Window window, string file) {
    var close = new Button { Content = "x" }; AutomationProperties.SetAutomationId(close, "CloseButton");
    var tab = new TabItem { Header = close, Tag = file }; AutomationProperties.SetName(tab, file + ". Unmodified.");
    close.Click += (s, e) => { tabs.Items.Remove(tab); if (tabs.Items.Count == 0) window.Close(); };
    return tab;
  }
}
'@

# Returns the UI Automation element of a process's window control with the given automation id. Inputs: process id, automation id.
function Get-Control($id, $automationId) {
  $w = Get-NotepadWindows $id | Select-Object -First 1
  $w.FindFirst("Descendants", (New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty, $automationId)))
}

try {
  New-Item -ItemType Directory -Path $root | Out-Null
  $env:APPDATA = $root; $env:TEMP = $root
  . (Join-Path $PSScriptRoot "..\lib\win32.ps1")
  . (Join-Path $PSScriptRoot "..\lib\notepad.ps1")

  $self = (Get-Process -Id $PID).ProcessName
  $before = @(Get-Process $self | ForEach-Object Id)
  $ids = Start-OwnedNotepad $null "never shown" $self
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
  $owned = @(Start-OwnedNotepad $file $part $name)
  $procs = @($owned | ForEach-Object { Get-Process -Id $_ })
  Check "started on its file: owns the launcher and its window child" (($owned.Count -eq 2) -and ((Get-NotepadNames $owned[1]) -contains "$part.txt - Notepad"))
  Stop-OwnedNotepad $owned $part
  Check "Stop-OwnedNotepad closes the owned tab and both processes end by themselves" (-not ($procs | Where-Object { -not $_.HasExited }))

  $part = "cue-notepad-check-" + [guid]::NewGuid()
  $file = Join-Path $root "$part.txt"; Set-Content $file ""
  $owned = @(Start-OwnedNotepad $file $part $name)
  $procs = @($owned | ForEach-Object { Get-Process -Id $_ })
  (Get-Control $owned[1] "AddButton").GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
  $null = Await "the later tab to arrive" { (Get-NotepadNames $owned[1]) -contains "my notes.txt. Unmodified." } 10000
  Check "a later inactive tab under the owned title: the window title is still owned" ([Win32]::Titles([uint32]$owned[1]) -contains "$part.txt - Notepad")
  Check "a later inactive tab under the owned title: no longer owned" ($null -eq (Get-OwnedNotepad $owned[0] $part))
  Stop-OwnedNotepad $owned $part
  $left = @(Get-NotepadNames $owned[1])
  Check "Stop-OwnedNotepad closes only the check's tab" ((-not ($left -like "*$part*")) -and ($left -contains "my notes.txt. Unmodified."))
  Check "Stop-OwnedNotepad leaves both processes running" (-not ($procs | Where-Object { $_.HasExited }))
} finally {
  Get-Process $name -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  $env:APPDATA = $saved.APPDATA; $env:TEMP = $saved.TEMP; $env:PATH = $saved.PATH
  # an ended stand-in keeps its exe locked for a moment
  $null = WaitUntil { Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue; -not (Test-Path $root) } 10000
}
Complete-Checks
