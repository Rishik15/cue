# File: scripts/lib/handles.ps1
# Purpose: count the handles of type Process another process holds, so bench can show a process that keeps handles to processes that already exited (ticket 43: one per settings open).
# Contents: the Handles class (Add-Type, skipped when it is already loaded) with ProcessCount (the count), Table (the system handle table) and ProcessType (the object type index of Process, read from a handle this script opens itself).
if (-not ("Handles" -as [type])) {
  Add-Type @'
using System; using System.Runtime.InteropServices;
/// Counts another process's Process handles from the system handle table (SystemExtendedHandleInformation, 40-byte entries on 64-bit Windows).
public class Handles {
  [DllImport("ntdll.dll")] static extern int NtQuerySystemInformation(int cls, IntPtr buf, int len, out int ret);
  [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
  [DllImport("kernel32.dll")] static extern uint GetCurrentProcessId();
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  const int ExtendedHandles = 64, Mismatch = unchecked((int)0xC0000004), Entry = 40, Header = 16;

  /// Returns the system handle table in a buffer the caller frees with Marshal.FreeHGlobal; throws on any status but success.
  static IntPtr Table() {
    int len = 1 << 22, ret;
    while (true) {
      IntPtr b = Marshal.AllocHGlobal(len);
      int status = NtQuerySystemInformation(ExtendedHandles, b, len, out ret);
      if (status == 0) return b;
      Marshal.FreeHGlobal(b);
      if (status != Mismatch) throw new InvalidOperationException("NtQuerySystemInformation failed: 0x" + status.ToString("X8"));
      len = Math.Max(len * 2, ret + (1 << 16));
    }
  }

  /// Returns the object type index of Process in a table: the type of a process handle this process opened to itself. Inputs: the table, that handle.
  static int ProcessType(IntPtr table, IntPtr own) {
    long count = Marshal.ReadInt64(table); ulong me = GetCurrentProcessId();
    for (long i = 0; i < count; i++) {
      IntPtr e = (IntPtr)((long)table + Header + i * Entry);
      if ((ulong)Marshal.ReadInt64(e, 8) == me && Marshal.ReadIntPtr(e, 16) == own) return (ushort)Marshal.ReadInt16(e, 30);
    }
    throw new InvalidOperationException("this process's own handle is not in the system handle table");
  }

  /// Returns how many handles of type Process the process holds. Input: its id.
  public static int ProcessCount(uint pid) {
    IntPtr own = OpenProcess(0x1000, false, GetCurrentProcessId());
    if (own == IntPtr.Zero) throw new InvalidOperationException("could not open this process");
    IntPtr table = Table();
    try {
      int type = ProcessType(table, own), n = 0; long count = Marshal.ReadInt64(table);
      for (long i = 0; i < count; i++) {
        IntPtr e = (IntPtr)((long)table + Header + i * Entry);
        if ((ulong)Marshal.ReadInt64(e, 8) == pid && (ushort)Marshal.ReadInt16(e, 30) == type) n++;
      }
      return n;
    } finally { Marshal.FreeHGlobal(table); CloseHandle(own); }
  }
}
'@
}
