//! File: origin.rs
//! Purpose: Decide whether a clipboard update was the copy Cue asked the target for, or somebody else's. A clipboard sequence number says that something changed, never who wrote it, so a selection is used only when every sign agrees.
//! Contents: Signs — the facts gathered around one copy; accepted — the pure rule over them; in_tree — whether a process descends from another; descends_from — the same question asked of the live process list.
//! The rule (R1): the sequence moved after Ctrl+C went out, the foreground is still the target, the clipboard's owner is the target's process or one of its children, and exactly one update arrived. Otherwise the capture is Unknown: nothing is used and nothing is restored over the newer data.
//! shortcut: a packaged app whose clipboard owner lives in another process tree than its frame window (the Windows Store app host) reads as Unknown, add its host to `accepted` if it matters.

use std::collections::HashMap;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

/// The deepest parent chain followed.
const DEPTH: usize = 16;

/// Everything seen around one copy.
pub struct Signs {
    /// The sequence number before Ctrl+C went out.
    pub before: u32,
    /// The sequence number read in the same open as the text.
    pub after: u32,
    /// Clipboard updates heard since the clipboard was admitted (the change listener is registered before the admission) up to the end of the read.
    pub updates: u32,
    /// The foreground window is still the target's.
    pub still_there: bool,
    /// The process that owns the clipboard (0 when it has no owner window).
    pub owner_pid: u32,
    /// The target's process.
    pub target_pid: u32,
    /// The owner process is a child of the target's.
    pub owner_in_tree: bool,
}

/// Whether the update is the target's copy: all four signs agree.
pub fn accepted(signs: &Signs) -> bool {
    let from_target = signs.owner_pid != 0 && (signs.owner_pid == signs.target_pid || signs.owner_in_tree);
    signs.after != signs.before && signs.updates == 1 && signs.still_there && from_target
}

/// Whether `pid` is `root` or descends from it through `parents` (child to parent), following at most `DEPTH` links.
pub fn in_tree(parents: &HashMap<u32, u32>, pid: u32, root: u32) -> bool {
    let mut at = pid;
    for _ in 0..DEPTH {
        if at == root {
            return true;
        }
        match parents.get(&at) {
            Some(&parent) if parent != at && parent != 0 => at = parent,
            _ => return false,
        }
    }
    false
}

/// Whether `pid` descends from `root` in the live process list; false when the list cannot be read. Safety: the snapshot handle is closed before returning; the entry is a local with its size set.
pub fn descends_from(pid: u32, root: u32) -> bool {
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return false };
        let mut parents = HashMap::new();
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
        in_tree(&parents, pid, root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signs() -> Signs {
        Signs { before: 4, after: 5, updates: 1, still_there: true, owner_pid: 10, target_pid: 10, owner_in_tree: false }
    }

    /// A single update from the target's own process, with focus unmoved, is the copy.
    #[test]
    fn one_update_from_the_target_is_the_copy() {
        assert!(accepted(&signs()));
        assert!(accepted(&Signs { owner_pid: 11, owner_in_tree: true, ..signs() }));
    }

    /// Each sign alone refuses the capture: unchanged sequence, a second update, moved focus, a foreign or unnamed owner.
    #[test]
    fn any_disagreeing_sign_makes_the_copy_unknown() {
        assert!(!accepted(&Signs { after: 4, ..signs() }));
        assert!(!accepted(&Signs { updates: 2, ..signs() }));
        assert!(!accepted(&Signs { updates: 0, ..signs() }));
        assert!(!accepted(&Signs { still_there: false, ..signs() }));
        assert!(!accepted(&Signs { owner_pid: 99, ..signs() }));
        assert!(!accepted(&Signs { owner_pid: 0, target_pid: 0, ..signs() }));
    }

    /// A process counts as the target's when a parent chain reaches it, and a cycle or a broken chain does not.
    #[test]
    fn a_process_tree_follows_parents_and_stops_at_loops() {
        let parents: HashMap<u32, u32> = [(3, 2), (2, 1), (7, 7), (8, 9)].into();
        assert!(in_tree(&parents, 3, 1));
        assert!(in_tree(&parents, 1, 1));
        assert!(!in_tree(&parents, 7, 1));
        assert!(!in_tree(&parents, 8, 1));
        assert!(!in_tree(&parents, 5, 1));
    }
}
