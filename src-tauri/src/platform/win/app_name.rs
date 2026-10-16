//! Purpose: Name the programs on the screen, so snippets and rules can be scoped to apps and the user can pick one from a list instead of guessing an executable's name.
//! Contents: list_apps — every program a person could name (installed and running); foreground_app — the executable's file name without `.exe`, lower case ("slack", "chrome"); empty when unknown; running_apps — every program that has a window on screen, with its display name.

use std::ffi::c_void;

use super::installed_apps;
use windows::core::{w, BOOL, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, GetAncestor, GetClassNameW, GetForegroundWindow, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowThreadProcessId, IsWindowVisible, GA_ROOTOWNER, GWL_EXSTYLE, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

/// A running program as the picker lists it: its executable's name and the name it shows.
struct Running {
    exe: String,
    name: String,
}

/// The full path of the program that owns `window`; None when it cannot be read (a protected process).
fn image_path(window: HWND) -> Option<String> {
    // SAFETY: a process handle opened for a name query and closed again; the buffer is ours.
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(window, Some(&mut pid));
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let read = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(process);
        read.ok().map(|()| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// `C:\Apps\WezTerm\wezterm-gui.exe` to `wezterm-gui`.
fn exe_name(path: &str) -> String {
    let file = path.rsplit(['\\', '/']).next().unwrap_or("");
    let lower = file.to_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
}

/// The path of the program in front. A store app's window belongs to the frame host, which stands for every store app; the app itself owns a child window of the class `Windows.UI.Core.CoreWindow`, so that one is asked.
fn foreground_path() -> Option<String> {
    // SAFETY: window handle queries.
    let window = unsafe { GetForegroundWindow() };
    let path = image_path(window)?;
    if exe_name(&path) != FRAME_HOST {
        return Some(path);
    }
    // SAFETY: as above.
    let core = unsafe { FindWindowExW(Some(window), None, w!("Windows.UI.Core.CoreWindow"), PCWSTR::null()) }.ok();
    Some(core.and_then(image_path).unwrap_or(path))
}

/// The process that hosts the windows of every store app.
const FRAME_HOST: &str = "applicationframehost";

pub fn foreground_app() -> String {
    foreground_path().map(|path| exe_name(&path)).unwrap_or_default()
}

/// The program's display name from its version information (`FileDescription`, else `ProductName`), or an empty text. Only the picker asks, so nothing is cached.
fn display_name(path: &str) -> String {
    version_string(path, "FileDescription").or_else(|| version_string(path, "ProductName")).unwrap_or_default()
}

fn version_string(path: &str, key: &str) -> Option<String> {
    let file: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: the version block is read into a buffer sized by Windows; the strings point into it and are copied before it goes.
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(file.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut block = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(file.as_ptr()), None, size, block.as_mut_ptr().cast()).ok()?;
        let (mut at, mut len): (*mut c_void, u32) = (std::ptr::null_mut(), 0);
        if !VerQueryValueW(block.as_ptr().cast(), w!("\\VarFileInfo\\Translation"), &mut at, &mut len).as_bool() || len < 4 {
            return None;
        }
        let [lang, page] = std::ptr::read_unaligned(at.cast::<[u16; 2]>());
        let query: Vec<u16> = format!("\\StringFileInfo\\{lang:04x}{page:04x}\\{key}").encode_utf16().chain(std::iter::once(0)).collect();
        if !VerQueryValueW(block.as_ptr().cast(), PCWSTR(query.as_ptr()), &mut at, &mut len).as_bool() || len == 0 {
            return None;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(at.cast::<u16>(), len as usize));
        Some(text.trim_end_matches('\0').trim().to_string()).filter(|t| !t.is_empty())
    }
}

/// DWM's cloak value for a window hidden by the shell: on another virtual desktop or, with a tiling manager such as GlazeWM, on a workspace that is not showing. Such a window is a real app window, only out of sight.
const CLOAKED_BY_SHELL: u32 = 2;
/// The class of the window store apps draw in; system pieces such as the input experience use it too, and the app's own frame window is what a person sees.
const CORE_WINDOW: &str = "Windows.UI.Core.CoreWindow";

fn class_of(window: HWND) -> String {
    let mut buf = [0u16; 64];
    // SAFETY: the buffer is ours; the call fills at most its length.
    let len = unsafe { GetClassNameW(window, &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

/// The tests that make a window one a person would call an app's, in order: visible, not hidden by Windows (a suspended store app is "cloaked"), its own root (no owner), not a tool window unless it asks to be listed, able to be
/// activated, and titled. This is the widely used approximation of what Alt+Tab lists; Windows' own rule is an implementation detail.
fn window_tests(window: HWND) -> [bool; 7] {
    // SAFETY: plain queries on a window handle EnumWindows gave us.
    unsafe {
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(window, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), 4);
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
        [
            IsWindowVisible(window).as_bool(),
            cloaked == 0 || cloaked == CLOAKED_BY_SHELL,
            GetAncestor(window, GA_ROOTOWNER) == window,
            style & WS_EX_TOOLWINDOW.0 == 0 || style & WS_EX_APPWINDOW.0 != 0,
            style & WS_EX_NOACTIVATE.0 == 0,
            GetWindowTextLengthW(window) > 0,
            class_of(window) != CORE_WINDOW,
        ]
    }
}

/// What the enumeration gathers: the windows that count as an app's, and for each test how many windows passed it.
#[derive(Default)]
struct Found {
    app_windows: Vec<HWND>,
    seen: usize,
    passed: [usize; 7],
}

unsafe extern "system" fn collect(window: HWND, found: LPARAM) -> BOOL {
    // SAFETY: `found` is the `Found` that `running_apps` passed in, alive for the whole enumeration.
    let found = unsafe { &mut *(found.0 as *mut Found) };
    found.seen += 1;
    let tests = window_tests(window);
    for (count, ok) in found.passed.iter_mut().zip(tests) {
        *count += usize::from(ok);
    }
    if tests.iter().all(|ok| *ok) {
        found.app_windows.push(window);
    }
    BOOL(1)
}

/// Every program that has a window on screen, once each; Cue itself is left out.
fn running_apps() -> Vec<Running> {
    let mut found = Found::default();
    // SAFETY: the callback only runs during this call and gets the pointer to `found`.
    let walked = unsafe { EnumWindows(Some(collect), LPARAM(&mut found as *mut Found as isize)) };
    let mut apps: Vec<Running> = Vec::new();
    let (seen, counted, passed) = (found.seen, found.app_windows.len(), found.passed);
    for path in found.app_windows.into_iter().filter_map(image_path) {
        let exe = exe_name(&path);
        // The host of store apps stands for all of them, so it says nothing about which one is meant.
        if exe == "cue" || exe == FRAME_HOST || apps.iter().any(|a| a.exe == exe) {
            continue;
        }
        let name = Some(display_name(&path)).filter(|n| !n.is_empty()).unwrap_or_else(|| exe.clone());
        apps.push(Running { exe, name });
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    crate::log::note(&format!(
        "running programs: {seen} windows seen, passing visible/uncloaked/root/listed/activatable/titled/not-core {passed:?}, {counted} app windows, {} programs (walk ok: {})",
        apps.len(),
        walked.is_ok()
    ));
    apps
}

/// Every program a person could name, as `Name\texe` (just `Name` when no executable is known): all those the Start menu lists, and the running ones that it does not (portable programs). Once each, sorted by name.
/// The saved form is what the focus check compares, so a picked program is told apart by its executable and not by a name.
pub fn list_apps() -> Vec<String> {
    let mut programs = installed_apps::programs();
    for app in running_apps() {
        let known = programs.iter().any(|p| p.exe.as_deref() == Some(app.exe.as_str()) || p.name.eq_ignore_ascii_case(&app.name));
        if !known {
            programs.push(installed_apps::Program { name: app.name, exe: Some(app.exe) });
        }
    }
    programs.sort_by_key(|p| p.name.to_lowercase());
    programs.into_iter().map(|p| p.exe.map_or(p.name.clone(), |exe| format!("{}\t{exe}", p.name))).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_picker_lists_programs_with_their_executables() {
        let apps = super::list_apps();
        assert!(!apps.is_empty() && apps.iter().all(|a| !a.split('\t').next().unwrap_or("").is_empty()));
        assert!(apps.iter().any(|a| a.split_once('\t').is_some_and(|(_, exe)| !exe.is_empty() && exe == exe.to_lowercase())));
    }
}
