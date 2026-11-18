/**
 * Purpose: The break screen's one door to Rust, every call naming the owner of its break so Rust ignores a page whose break was replaced (`break_act` in `src-tauri/src/notes/breaks/mod.rs`).
 * Contents: callBreak — for `owner`: it painted ("ready"), Sneak a peek ("peek") or End Pomodoro ("end").
 */
import { invoke } from "@tauri-apps/api/core";
import type { Owner } from "./types";

export const callBreak = (act: "ready" | "peek" | "end", owner: Owner) => void invoke("break_act", { act, owner });
