/**
 * File: api.ts
 * Purpose: The break screen's one door to Rust, every call naming the serial of its break so Rust ignores a page whose break was replaced or followed by a later one (`break_act` in `src-tauri/src/notes/breaks/mod.rs`).
 * Contents: callBreak — for break `serial`: it painted ("ready"), Sneak a peek ("peek") or End Pomodoro ("end").
 */
import { invoke } from "@tauri-apps/api/core";

/** Sends `act` of break `serial` to Rust. */
export const callBreak = (act: "ready" | "peek" | "end", serial: number) => void invoke("break_act", { act, serial });
