# File: scripts/lib/win32.ps1
# Purpose: one set of Win32 calls (key and mouse injection, window lookup and visibility, foreground window, window rectangles, GUI resource counts, window listing) shared by the verify and lab harnesses, so each harness does not declare its own one-letter copy.
# Contents: the Win32 class (Add-Type, skipped when it is already loaded) and its nested RECT struct.
if (-not ("Win32" -as [type])) {
  Add-Type @'
using System; using System.Runtime.InteropServices;
/// Win32 calls the harnesses use to drive Cue like a person: inject keys and clicks, find and inspect windows, read GUI object counts.
public class Win32 {
  /// Injects a key event. Inputs: virtual key, scan code, flags (0 down, 2 up, +1 extended key), extra info.
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  /// Injects a mouse event. Inputs: flags (2 left down, 4 left up), dx, dy, data, extra info.
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
  /// Finds a top-level window by class and/or title (null matches any). Returns its handle, or zero.
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
  /// Returns whether a window is visible. Input: window handle.
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  /// Posts a message to a window's queue. Inputs: handle, message, wparam, lparam.
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  /// Returns the handle of the window that holds the keyboard focus.
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  /// Reads a window's rectangle in screen pixels. Inputs: handle, the rectangle to fill.
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  /// Makes this process report real pixels instead of scaled ones, so coordinates match the screen.
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  /// Counts GUI objects of a process. Inputs: process handle, 0 for GDI objects or 1 for USER objects.
  [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flags);
  delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, System.Text.StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  /// A window rectangle in screen pixels.
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  /// Returns whether a process owns a top-level window of a class. Inputs: process id, window class name.
  public static bool OwnsWindow(uint pid, string cls) {
    bool found = false;
    EnumWindows((h, l) => {
      uint owner; GetWindowThreadProcessId(h, out owner);
      if (owner != pid) return true;
      var c = new System.Text.StringBuilder(128); GetClassName(h, c, 128);
      if (c.ToString() == cls) { found = true; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
  /// Lists every top-level window the given processes own as "pid class title visible=flag". Input: process ids.
  public static System.Collections.Generic.List<string> WindowsOf(uint[] pids) {
    var list = new System.Collections.Generic.List<string>();
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (Array.IndexOf(pids, pid) >= 0) {
        var c = new System.Text.StringBuilder(128); var t = new System.Text.StringBuilder(128);
        GetClassName(h, c, 128); GetWindowText(h, t, 128);
        list.Add(pid + " " + c + " \"" + t + "\" visible=" + IsWindowVisible(h));
      }
      return true;
    }, IntPtr.Zero);
    return list;
  }
}
'@
}
