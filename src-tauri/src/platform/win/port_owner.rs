//! Purpose: Tell which process listens on a local TCP port, so Cue can check that the text engine on its loopback port is its own child and not a program that took the port first.
//! Contents: port_owner — the process id listening on an IPv4 port, or None.

use windows::Win32::Foundation::NO_ERROR;
use windows::Win32::NetworkManagement::IpHelper::{GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER};
use windows::Win32::Networking::WinSock::AF_INET;

pub fn port_owner(port: u16) -> Option<u32> {
    let mut len = 0u32;
    // The first call only reports the size it needs, and fails by design.
    unsafe { GetExtendedTcpTable(None, &mut len, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_LISTENER, 0) };
    let mut buf = vec![0u8; len as usize];
    let table = buf.as_mut_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
    let result = unsafe { GetExtendedTcpTable(Some(table.cast()), &mut len, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_LISTENER, 0) };
    if result != NO_ERROR.0 {
        return None;
    }
    // The port is stored in network byte order in the low 16 bits.
    let rows = unsafe { std::slice::from_raw_parts((*table).table.as_ptr(), (*table).dwNumEntries as usize) };
    rows.iter().find(|r| u16::from_be(r.dwLocalPort as u16) == port).map(|r| r.dwOwningPid)
}
