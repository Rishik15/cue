//! Purpose: What machine Cue runs on, for the log and for choices that depend on memory.
//! Contents: weak_machine — under 4 logical processors or under 6 GB, where Cue picks lighter models and waits longer; total_ram_mb — installed memory in MB (0 when unreadable); summary — one line: CPU name, logical and fast cores, RAM, Windows build.

use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

pub fn total_ram_mb() -> u64 {
    let mut status = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    if unsafe { GlobalMemoryStatusEx(&mut status) }.is_err() {
        return 0;
    }
    status.ullTotalPhys >> 20
}

pub fn weak_machine() -> bool {
    let ram = total_ram_mb();
    std::thread::available_parallelism().map_or(true, |n| n.get() < 4) || (ram != 0 && ram < 6144)
}

/// A text value under `HKLM\HARDWARE` or `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`, empty when missing.
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

pub fn summary() -> String {
    let cpu = registry_text(w!(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0"), w!("ProcessorNameString"));
    let build = registry_text(w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"), w!("CurrentBuildNumber"));
    let logical = std::thread::available_parallelism().map_or(0, |n| n.get());
    format!(
        "machine: {cpu}, {logical} logical, {} fast cores, {} MB RAM, Windows build {build}",
        super::performance_cores(),
        total_ram_mb()
    )
}
