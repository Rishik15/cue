//! File: process.rs
//! Purpose: Process and CPU facts the speech worker needs: how many fast cores to use, and opting out of Windows' background throttling.
//! Contents: performance_cores — physical cores of the fastest efficiency class; disable_power_throttling — turn EcoQoS off for this process; background_thread — low CPU and disk priority for the calling thread;
//! process_killer — a function that terminates a child process; hide_console — no console window for a child process; background_engine — the same at below-normal priority for the engines; end_with_parent — a child that must not outlive Cue; harden_process — restrict where DLLs load from; count_fast — pure core counting; cpu_sets — the CPU sets Windows reports; job — the kill-on-close job object.

use std::collections::HashSet;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};
use std::sync::OnceLock;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::{SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOOPENFILEERRORBOX};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::LibraryLoader::{SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS};
use windows::Win32::System::SystemInformation::{CpuSetInformation, GetSystemCpuSetInformation, SYSTEM_CPU_SET_INFORMATION};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThread, ProcessPowerThrottling, SetProcessInformation, SetThreadPriority, TerminateProcess,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
    THREAD_MODE_BACKGROUND_BEGIN,
};

/// Process creation flag: no console window.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Process creation flag: below-normal priority.
const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;

/// `(processor group, core index, efficiency class)` per logical processor -> physical cores in the highest class (P-cores on hybrid CPUs). The core index only counts within its group, so the group is part of a core's identity.
fn count_fast(sets: &[(u16, u8, u8)]) -> usize {
    let best = sets.iter().map(|s| s.2).max().unwrap_or(0);
    sets.iter().filter(|s| s.2 == best).map(|s| (s.0, s.1)).collect::<HashSet<_>>().len()
}

/// `(processor group, core index, efficiency class)` per logical processor as Windows reports them, CPU-set records only; empty when it will not say. The first call only reports the buffer size it needs, and fails by design.
/// Safety: the buffer is sized by Windows, records are read unaligned and only while `at + size` stays inside it.
fn cpu_sets() -> Vec<(u16, u8, u8)> {
    let process = unsafe { GetCurrentProcess() };
    let mut len = 0u32;
    let _ = unsafe { GetSystemCpuSetInformation(None, 0, &mut len, Some(process), None) };
    let mut buf = vec![0u8; len as usize];
    let first = buf.as_mut_ptr() as *mut SYSTEM_CPU_SET_INFORMATION;
    let filled = unsafe { GetSystemCpuSetInformation(Some(first), len, &mut len, Some(process), None) }.as_bool();
    if len == 0 || !filled {
        return Vec::new();
    }
    let (mut at, mut sets) = (0usize, Vec::new());
    while at + std::mem::size_of::<SYSTEM_CPU_SET_INFORMATION>() <= len as usize {
        let rec = unsafe { std::ptr::read_unaligned(buf.as_ptr().add(at) as *const SYSTEM_CPU_SET_INFORMATION) };
        if rec.Size == 0 {
            break;
        }
        if rec.Type == CpuSetInformation {
            let cpu = unsafe { rec.Anonymous.CpuSet };
            sets.push((cpu.Group, cpu.CoreIndex, cpu.EfficiencyClass));
        }
        at += rec.Size as usize;
    }
    sets
}

/// The number of fast cores to use. Falls back to the logical processor count when Windows will not say.
pub fn performance_cores() -> usize {
    match count_fast(&cpu_sets()) {
        0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
        n => n,
    }
}

/// Windows moves a window-less background process to efficiency cores at low clocks (EcoQoS): measured 2x slower inference. Safety: `state` outlives the call and `size` is its size.
pub fn disable_power_throttling() {
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: 0,
    };
    let size = std::mem::size_of_val(&state) as u32;
    let _ = unsafe { SetProcessInformation(GetCurrentProcess(), ProcessPowerThrottling, &state as *const _ as _, size) };
}

/// Low CPU and disk priority for the calling thread until it ends, so a background download never slows the PC or what the user is doing. Safety: `GetCurrentThread` is a pseudo handle for the calling thread and needs no closing; the call has no other pointers.
pub fn background_thread() {
    let _ = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
}

/// Search only the application folder, System32 and added directories for DLLs, never the current directory (a classic planting spot).
/// Also turns off Windows' error dialogs for this process and the engines it starts (children inherit it): a blocked or broken DLL then fails the
/// start quietly and Cue reports it, instead of a "Bad Image" box nobody can act on. Safety: process-wide flags, no pointers.
pub fn harden_process() {
    let _ = unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS) };
    unsafe { SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX) };
}

/// Starts `command` without a console window.
pub fn hide_console(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

/// No console window, and below-normal priority: an engine (speech, text model) that saturates a small PC must not starve the keyboard hook, the webview and the app it types into.
pub fn background_engine(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
}

/// One job for the whole app (kept as an integer: a raw handle is not `Sync`). Windows kills every process in it when Cue exits,
/// however Cue exits, so a model server cannot be left running after a crash. Safety: the job handle lives for the whole process, or is closed here when it could not be configured.
fn job() -> Option<HANDLE> {
    static JOB: OnceLock<isize> = OnceLock::new();
    let raw = *JOB.get_or_init(|| {
        let Ok(job) = (unsafe { CreateJobObjectW(None, None) }) else { return 0 };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size = std::mem::size_of_val(&info) as u32;
        let set = unsafe { SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info as *const _ as _, size) };
        if set.is_ok() {
            return job.0 as isize;
        }
        let _ = unsafe { CloseHandle(job) };
        0
    });
    (raw != 0).then_some(HANDLE(raw as _))
}

/// Ties `child` to Cue's lifetime. False when Windows refused: the child would outlive a crashed Cue, so the caller decides whether to keep it. Safety: both handles are valid for the call.
pub fn end_with_parent(child: &Child) -> bool {
    let Some(job) = job() else { return false };
    unsafe { AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())) }.is_ok()
}

/// A function that terminates `child` at once and may be called from any thread, any number of times. It holds the child's process handle by value, so the caller must keep the `Child` alive for as long as the function can still run.
pub fn process_killer(child: &Child) -> impl Fn() + Send + Sync + 'static {
    let handle = child.as_raw_handle() as usize;
    move || {
        let _ = unsafe { TerminateProcess(HANDLE(handle as *mut _), 1) };
    }
}

#[cfg(test)]
mod tests {
    use super::count_fast;

    /// The same core index in two processor groups is two cores.
    #[test]
    fn same_core_index_in_two_groups_is_two_cores() {
        assert_eq!(count_fast(&[(0, 0, 1), (1, 0, 1)]), 2);
    }

    /// Hyper-thread siblings count once, and efficiency cores are left out.
    #[test]
    fn smt_siblings_count_once_and_efficiency_cores_are_left_out() {
        assert_eq!(count_fast(&[(0, 0, 1), (0, 0, 1), (0, 1, 1), (0, 2, 0)]), 2);
        assert_eq!(count_fast(&[]), 0);
    }
}
