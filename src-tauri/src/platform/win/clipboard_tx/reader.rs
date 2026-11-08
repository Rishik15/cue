//! File: reader.rs
//! Purpose: Tell who is reading the clipboard when a delayed-render request arrives: the paste's target, a clipboard agent, or Cue's own clipboard watcher. Only the target's read proves a paste; an agent's read does not (ADR 0014); Cue's own watcher is neither.
//! Contents: Reader — the three kinds; classify — pure, from the reader's (top-level window, process), the target's and Cue's own process id.

/// Who is reading the clipboard when a render request comes.
#[derive(Debug, PartialEq, Eq)]
pub enum Reader {
    /// The paste's target.
    Target,
    /// A history tool, a clipboard agent or a reader that cannot be named.
    Agent,
    /// Another window of Cue itself (its clipboard watcher): not a program that took the text.
    Cue,
}

/// Classifies a reader from its (top-level window, process) against the paste's `target` and Cue's own process `own`. A reader that cannot be named is an agent, never the target; the target wins over Cue's own process.
pub fn classify(reader: Option<(isize, u32)>, target: (isize, u32), own: u32) -> Reader {
    let Some((reader_root, reader_pid)) = reader else { return Reader::Agent };
    let (root, pid) = target;
    if root != 0 && (reader_root == root || (pid != 0 && reader_pid == pid)) {
        Reader::Target
    } else if reader_pid == own {
        Reader::Cue
    } else {
        Reader::Agent
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, Reader};

    /// A reader in the target's window or process is the target; Cue's own process is not an agent; an unnamed or foreign reader is an agent.
    #[test]
    fn readers_are_told_apart() {
        let target = (100, 7);
        assert_eq!(classify(Some((100, 9)), target, 1), Reader::Target);
        assert_eq!(classify(Some((200, 7)), target, 1), Reader::Target);
        assert_eq!(classify(Some((300, 1)), target, 1), Reader::Cue);
        assert_eq!(classify(Some((300, 5)), target, 1), Reader::Agent);
        assert_eq!(classify(None, target, 1), Reader::Agent);
        assert_eq!(classify(Some((300, 1)), (0, 0), 1), Reader::Cue);
    }
}
