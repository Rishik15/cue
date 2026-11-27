/**
 * File: useUpdate.ts
 * Purpose: The update state the About page and the update window share: the background download's snapshot, plus checking for an update and installing it.
 * Contents: update — signals for the snapshot (phase, version, percent), the Check Now round trip and the brief "Up to Date" confirmation; watchUpdates — reads the status and listens for changes from Rust; check / install — the two actions (a failure shows as a toast and the state steps back).
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createSignal, untrack } from "solid-js";
import { isNum, isStr, nullable, shaped } from "./ipc";
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
/** How long a successful install has to quit Cue before the button comes back. */
const INSTALL_GRACE_MS = 5000;

/** Validates an event or command payload at the boundary. Input: the payload. Return: the snapshot with its percent kept within 0 to 100, or null when it is not one. */
function parse(raw: unknown): Snapshot | null {
  if (!shaped<Snapshot>(raw, { phase: (p) => PHASES.some((x) => x === p), version: nullable(isStr), percent: isNum })) return null;
  return { ...raw, percent: Math.min(100, Math.max(0, raw.percent)) };
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

/** What the user sees when the update check answers with something Cue cannot read. */
const MALFORMED = "Cue could not read the update check. Try again.";

/** Checks for an update now: "Checking…" for at least a second, then "Up to Date" for a moment when there is none; a refusal or a malformed answer shows as a toast. Return: resolves when the check is over. */
export function check() {
  setChecking(true);
  const shown = new Promise((done) => setTimeout(done, CHECKING_MS));
  return invoke<unknown>("check_update")
    .finally(() => shown)
    .then((raw) => {
      const next = apply(raw);
      if (!next) return showToast(MALFORMED);
      if (next.phase !== "none") return;
      setLatest(true);
      clearTimeout(latestTimer);
      latestTimer = setTimeout(() => setLatest(false), LATEST_MS);
    }, (e) => showToast(String(e)))
    .finally(() => setChecking(false));
}

/** Installs the downloaded update; Cue quits when it works, and the button comes back when it does not. Return: resolves when the call is over. */
export function install() {
  const before = snapshot();
  setSnapshot({ ...before, phase: "installing" });
  // A successful install quits Cue; if it is still here a few seconds later, the button comes back instead of staying on "Updating…" for good.
  return invoke("install_update").then(() => setTimeout(() => setSnapshot(before), INSTALL_GRACE_MS), (e) => {
    setSnapshot(before);
    showToast(String(e));
  });
}
