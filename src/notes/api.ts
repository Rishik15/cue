/**
 * File: notes/api.ts
 * Purpose: The note window's one door to Rust: a single command that takes what to do (`Cmd` in `src-tauri/src/notes/mod.rs`, mirrored by `NoteCmd`) and answers with the note to show, the rows of a list, or nothing.
 * Contents: NoteCmd — every command the page may send; callNote — send one command.
 */
import { invoke } from "@tauri-apps/api/core";

/** Every command the page may send, as Rust's `Cmd` takes it (tagged by `do`), so a misspelled command or field fails the type check. */
export type NoteCmd =
  | { do: "save"; id: number; body: string }
  | { do: "go"; from: number; to: "older" | "newer" | "newest" | "new" }
  | { do: "open"; from: number; id: number }
  | { do: "trash" | "restore" | "promote" | "close"; id: number }
  | { do: "pin"; id: number; pinned: boolean }
  | { do: "list"; query: string; trash: boolean }
  | { do: "eval"; text: string; digits: number; thousands: boolean }
  | { do: "timer"; id: number; line: string | null }
  | { do: "timerctl"; id: number; ctl: "pause" | "restart" | "stop" }
  | { do: "link"; url: string }
  | { do: "collect"; on: boolean }
  | { do: "image"; path: string }
  | { do: "ready" | "focused" };

/** Sends one command. Input: the command. Return: Rust's reply, unchecked (each caller checks the shape it expects). */
export const callNote = (cmd: NoteCmd) => invoke<unknown>("note", { cmd });
