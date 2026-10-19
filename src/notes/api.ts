/**
 * Purpose: The note window's one door to Rust: a single command that takes what to do (`Cmd` in `src-tauri/src/notes/mod.rs`) and answers with the note to show, the rows of a list, or nothing.
 * Contents: callNote — send one command.
 */
import { invoke } from "@tauri-apps/api/core";

export const callNote = (cmd: { do: string } & Record<string, unknown>) => invoke<unknown>("note", { cmd });
