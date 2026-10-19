/**
 * Purpose: The update state the About page and the update window share: the background download's snapshot, plus checking for an update and installing it.
 * Contents: update — signals for the snapshot (phase, version, percent), the Check Now round trip and the brief "Up to Date" confirmation; watchUpdates — reads the status and listens for changes from Rust; check / install — the two actions (a failure shows as a toast and the state steps back).
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createSignal, untrack } from "solid-js";
import { showToast } from "./ui/Toast";

const PHASES = ["none", "downloading", "ready", "installing"] as const;
export type Snapshot = { phase: (typeof PHASES)[number]; version: string | null; percent: number };

const [snapshot, setSnapshot] = createSignal<Snapshot>({ phase: "none", version: null, percent: 0 });
const [checking, setChecking] = createSignal(false);
const [latest, setLatest] = createSignal(false);
let latestTimer: ReturnType<typeof setTimeout> | undefined;

export const update = { snapshot, checking, latest };

/** "Up to Date" shows for a moment on the button, then it offers Check Now again. */
const LATEST_MS = 3000;
/** "Checking…" stays up at least this long, so a quick answer does not flash past. */
const CHECKING_MS = 1000;
const INSTALL_GRACE_MS = 5000;

/** Validates an event or command payload at the boundary; null when it is not a Snapshot. */
function parse(raw: unknown): Snapshot | null {
  if (typeof raw !== "object" || raw === null) return null;
  const { phase, version, percent } = raw as Record<string, unknown>;
  const known = PHASES.find((p) => p === phase);
  if (!known || typeof percent !== "number" || !Number.isFinite(percent)) return null;
  if (version !== null && typeof version !== "string") return null;
  return { phase: known, version, percent: Math.min(100, Math.max(0, percent)) };
}

/** Applies a payload; once installing, news from Rust must not bring the button back. */
function apply(raw: unknown) {
  const next = parse(raw);
  if (next && untrack(snapshot).phase !== "installing") setSnapshot(next);
  return next;
}

/** Mounted once with the window. Returns the function that stops listening. */
export function watchUpdates() {
  // Listen first so an event between the two calls is not lost.
  const off = listen<unknown>("cue-update", (e) => void apply(e.payload));
  void off.then(() => invoke<unknown>("update_status")).then(apply, () => undefined);
  return () => void off.then((unlisten) => unlisten());
}

export function check() {
  setChecking(true);
  const shown = new Promise((done) => setTimeout(done, CHECKING_MS));
  return invoke<unknown>("check_update")
    .finally(() => shown)
    .then((raw) => {
      if (apply(raw)?.phase !== "none") return;
      setLatest(true);
      clearTimeout(latestTimer);
      latestTimer = setTimeout(() => setLatest(false), LATEST_MS);
    }, (e) => showToast(String(e)))
    .finally(() => setChecking(false));
}

export function install() {
  const before = snapshot();
  setSnapshot({ ...before, phase: "installing" });
  // A successful install quits Cue; if it is still here a few seconds later, the button comes back instead of staying on "Updating…" for good.
  return invoke("install_update").then(() => setTimeout(() => setSnapshot(before), INSTALL_GRACE_MS), (e) => {
    setSnapshot(before);
    showToast(String(e));
  });
}
