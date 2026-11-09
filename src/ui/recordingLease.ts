/**
 * File: recordingLease.ts
 * Purpose: The bookkeeping of one shortcut control's recording, apart from any signal: the request number the page chose (so a cancel works before Rust has answered the open), the ordering of Rust's views, a fresh snapshot after a gap, freeing Rust's handle when a recording ends by itself, and the window watchers while it runs.
 * Contents: RecorderView — the shape; isView — validates a view from Rust; viewProblem — the message a view carries, if any; nextRequest — a request number higher than any before, also across a page reload; watchWindow — the page keeps its hands off the keys (Escape still cancels, in case the native hook was lost); makeLease — the lease bookkeeping.
 */
import { invoke } from "@tauri-apps/api/core";
import type { Part } from "./KeyChip";
import { watchRegions } from "./shortcutRegions";

type RecorderEvent = { kind: "none" | "saved" | "invalid" | "cancelled" | "failed"; message?: string };
export type RecorderView = { lease: number; seq: number; open: boolean; parts: Part[]; error: string | null; event: RecorderEvent };

const KEYS = ["keydown", "keyup", "keypress"] as const;

/** Checks a view from Rust before it is used. */
export const isView = (v: unknown): v is RecorderView => {
  const r = v as RecorderView | null;
  return typeof r === "object" && r !== null && typeof r.lease === "number" && typeof r.seq === "number" && typeof r.open === "boolean" && Array.isArray(r.parts);
};

/** The problem a view reports: the message of a rejected combination, else the error of the recording itself, else an empty string. */
export const viewProblem = (v: RecorderView) => (v.event.kind === "invalid" ? v.event.message : v.error) ?? "";

let lastRequest = 0;
/** A request number higher than every one before it, including those of an earlier load of the page (the clock part), so Rust's cancel watermark never refuses a new open. */
export const nextRequest = () => (lastRequest = Math.max(lastRequest + 1, Date.now() * 1000));

/** While a shortcut is recorded the webview must not act on keys (Rust's hook records them). A click outside the control and its popup, or Escape reaching the page (the native hook did not take it), calls `leave`. Returns the function that stops watching. */
function watchWindow(leave: () => void) {
  const click = (e: MouseEvent) => {
    if (!(e.target instanceof Element) || !e.target.closest("[data-shortcut-recording]")) leave();
  };
  const swallow = (e: Event) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.type === "keydown" && (e as KeyboardEvent).key === "Escape") leave();
  };
  window.addEventListener("click", click);
  KEYS.forEach((k) => window.addEventListener(k, swallow, true));
  return () => {
    window.removeEventListener("click", click);
    KEYS.forEach((k) => window.removeEventListener(k, swallow, true));
  };
}

/** The lease bookkeeping of one control. `show` receives each view in order; `ended` runs when the recording is over. */
export function makeLease(show: (v: RecorderView) => void, ended: () => void) {
  let request = 0;
  let lease = 0;
  let last = 0;
  let waiting = false;
  let early: RecorderView[] = [];
  let stops: Array<() => void> = [];
  const free = () => { if (request) void invoke("shortcut_record_cancel", { request }).catch(() => undefined); };
  const end = () => { stops.forEach((f) => f()); stops = []; request = 0; lease = 0; waiting = false; early = []; ended(); };
  const cancel = () => { free(); end(); };
  const deliver = (v: RecorderView) => { last = v.seq; show(v); if (!v.open) cancel(); };
  const resnapshot = () => invoke<unknown>("shortcut_resnapshot", { request }).then((r) => { if (isView(r) && r.lease === lease) deliver(r); }, () => undefined);
  return {
    cancel,
    end,
    /** Takes a view from the event stream: in order it is shown, a gap asks Rust for a fresh snapshot, a view of a lease not known yet is kept only while the open is on its way. */
    apply(v: RecorderView) {
      if (waiting && lease === 0) { early.push(v); return; }
      if (v.lease !== lease || v.seq <= last) return;
      if (v.seq === last + 1) deliver(v); else void resnapshot();
    },
    /** Opens the recording of `id`; throws Rust's sentence when it cannot start. A cancel made before Rust answers still reaches Rust, by the request number. */
    async begin(id: string, isOpen: () => boolean) {
      request = nextRequest();
      const mine = request;
      waiting = true;
      early = [];
      stops = [watchWindow(cancel)];
      const first = await invoke<unknown>("shortcut_record_open", { id, request: mine });
      if (!isView(first)) throw new Error("Cue could not start recording.");
      if (request !== mine || !isOpen()) return;
      lease = first.lease;
      last = 0;
      waiting = false;
      const newer = early.filter((v) => v.lease === lease && v.seq > first.seq).sort((a, b) => b.seq - a.seq)[0];
      early = [];
      deliver(newer ?? first);
      if (lease) stops.push(watchRegions(mine));
    },
  };
}
