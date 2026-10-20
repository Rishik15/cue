/**
 * Purpose: The break screen's one door to Rust (`break_act` in `src-tauri/src/notes/breaks/mod.rs`).
 * Contents: callBreak — it painted ("ready"), Sneak a peek ("peek") or End Pomodoro ("end").
 */
import { invoke } from "@tauri-apps/api/core";

export const callBreak = (act: "ready" | "peek" | "end") => void invoke("break_act", { act });
