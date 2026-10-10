//! Purpose: Process and CPU facts the speech worker needs: how many fast cores to use, and opting out of Windows' background throttling.
//! Contents: performance_cores — physical cores of the fastest efficiency class; disable_power_throttling — turn EcoQoS off for this process;
//! hide_console — no console window for a child process; count_fast — pure core counting (tested).

use std::collections::HashSet;
use std::os::windows::process::CommandExt;
use std::process::Command;

use windows::Win32::System::SystemInformation::{GetSystemCpuSetInformation, SYSTEM_CPU_SET_INFORMATION};
use windows::Win32::System::Threading::{
    GetCurrentProcess, ProcessPowerThrottling, SetProcessInformation, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `(core index, efficiency class)` per logical processor -> physical cores in the highest class (P-cores on hybrid CPUs).
fn count_fast(sets: &[(u8, u8)]) -> usize {
    let best = sets.iter().map(|s| s.1).max().unwrap_or(0);
    sets.iter().filter(|s| s.1 == best).map(|s| s.0).collect::<HashSet<_>>().len()
}

fn cpu_sets() -> Vec<(u8, u8)> {
    let process = unsafe { GetCurrentProcess() };
    let mut len = 0u32;
    // The first call only reports the buffer size it needs, and fails by design.
    let _ = unsafe { GetSystemCpuSetInformation(None, 0, &mut len, Some(process), None) };
    let mut buf = vec![0u8; len as usize];
    let first = buf.as_mut_ptr() as *mut SYSTEM_CPU_SET_INFORMATION;
    let filled = unsafe { GetSystemCpuSetInformation(Some(first), len, &mut len, Some(process), None) }.as_bool();
    if len == 0 || !filled {
        return Vec::new();
    }
    let (mut at, mut sets) = (0usize, Vec::new());
    while at + std::mem::size_of::<u32>() * 2 <= len as usize {
        let rec = unsafe { std::ptr::read_unaligned(buf.as_ptr().add(at) as *const SYSTEM_CPU_SET_INFORMATION) };
        if rec.Size == 0 {
            break;
        }
        let cpu = unsafe { rec.Anonymous.CpuSet };
        sets.push((cpu.CoreIndex, cpu.EfficiencyClass));
        at += rec.Size as usize;
    }
    sets
}

/// Falls back to the logical processor count when Windows will not say.
pub fn performance_cores() -> usize {
    match count_fast(&cpu_sets()) {
        0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
        n => n,
    }
}

/// Windows moves a window-less background process to efficiency cores at low clocks (EcoQoS): measured 2x slower inference.
pub fn disable_power_throttling() {
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: 0,
    };
    let size = std::mem::size_of_val(&state) as u32;
    let _ = unsafe { SetProcessInformation(GetCurrentProcess(), ProcessPowerThrottling, &state as *const _ as _, size) };
}

pub fn hide_console(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(test)]
mod tests {
    use super::count_fast;

    #[test]
    fn counts_physical_cores_of_best_class() {
        // 4 P-cores with SMT (class 1, 8 threads) and 4 E-cores (class 0).
        let sets: Vec<(u8, u8)> = (0..8).map(|i| (i / 2, 1)).chain((4..8).map(|i| (i, 0))).collect();
        assert_eq!(count_fast(&sets), 4);
        assert_eq!(count_fast(&[]), 0);
    }
}
