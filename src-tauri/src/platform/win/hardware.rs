//! File: hardware.rs
//! Purpose: What machine Cue runs on, for the log and for choices that depend on memory.
//! Contents:
//! - remote_session — inside a Remote Desktop session
//! - weak_machine — under 4 logical processors or under 6 GB, where Cue picks lighter models and waits longer
//! - virtual_machine — whether the BIOS names a hypervisor's virtual machine (read once)
//! - total_ram_mb — installed memory in MB (0 when unreadable)
//! - summary — one line: CPU name, logical and fast cores, RAM, Windows build, virtual machine or not

use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_REMOTESESSION};

use crate::platform::typing_policy::is_virtual_machine;

/// Installed physical memory in MB. Return: 0 when Windows cannot say. Safety: `GlobalMemoryStatusEx` fills a correctly sized struct owned here.
pub fn total_ram_mb() -> u64 {
    let mut status = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    if unsafe { GlobalMemoryStatusEx(&mut status) }.is_err() {
        return 0;
    }
    status.ullTotalPhys >> 20
}

/// True inside a Remote Desktop session, where injected input is known to be dropped when it comes in one burst. Safety: Reads one system metric.
pub fn remote_session() -> bool {
    unsafe { GetSystemMetrics(SM_REMOTESESSION) != 0 }
}

/// Whether this is a small machine: fewer than four logical processors or under 6 GB of memory. It says nothing about virtual machines (see `virtual_machine`).
pub fn weak_machine() -> bool {
    let ram = total_ram_mb();
    std::thread::available_parallelism().map_or(true, |n| n.get() < 4) || (ram != 0 && ram < 6144)
}

/// Whether Windows runs in a virtual machine, by the BIOS manufacturer and product names; read once, then remembered. A physical PC with Hyper-V turned on reports its real vendor and is not counted.
pub fn virtual_machine() -> bool {
    static VM: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *VM.get_or_init(|| {
        let key = w!(r"HARDWARE\DESCRIPTION\System\BIOS");
        is_virtual_machine(&registry_text(key, w!("SystemManufacturer")), &registry_text(key, w!("SystemProductName")))
    })
}

/// A text value under `HKLM\HARDWARE` or `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`, empty when missing. Safety: the buffer and size passed to the registry read are owned here.
fn registry_text(key: PCWSTR, value: PCWSTR) -> String {
    let mut buf = [0u16; 128];
    let mut size = size_of_val(&buf) as u32;
    let status =
        unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key, value, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size)) };
    if status.0 != 0 {
        return String::new();
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len]).trim().to_string()
}

/// A one-line description of the machine (processor, Windows build, logical processors, memory) for the log.
pub fn summary() -> String {
    let cpu = registry_text(w!(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0"), w!("ProcessorNameString"));
    let build = registry_text(w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"), w!("CurrentBuildNumber"));
    let logical = std::thread::available_parallelism().map_or(0, |n| n.get());
    format!(
        "machine: {cpu}, {logical} logical, {} fast cores, {} MB RAM, Windows build {build}, virtual machine {}",
        super::performance_cores(),
        total_ram_mb(),
        virtual_machine()
    )
}
