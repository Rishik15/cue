/**
 * File: useRules.ts
 * Purpose: State behind the Rules page: the saved rules, the action catalogue, and saving with the reason when Rust refuses.
 * Contents: useRules — signals plus update (change the list; resolves true when stored; refused until the saved rules have been read and for good when they could not be,
 * so a save never replaces a file Cue has not loaded). Updates run one after another, each on the list as the
 * previous one left it, so switching two rules quickly cannot undo the first.
 */
import { createSignal, onMount } from "solid-js";
import { reason } from "./ipc";
import { getRules, listActions, saveRules, type ActionInfo, type Rule } from "./rules";

/** What a change before the saved rules are read says; cleared once they arrive. */
const LOADING = "Still loading your actions. Try again in a moment.";

/** The Rules page state. Return: the rules, the action catalogue, the last error, `update`, which applies a change to the list and resolves whether it was stored (a change before the rules are read says so in the error), and `busy`, true while an update is queued. */
export function useRules() {
  const [rules, setRules] = createSignal<Rule[]>([]);
  const [actions, setActions] = createSignal<ActionInfo[]>([]);
  const [error, setError] = createSignal("");
  let loaded: "loading" | "ready" | "failed" = "loading";
  onMount(() => {
    getRules().then((r) => { setRules(r); loaded = "ready"; setError((e) => (e === LOADING ? "" : e)); }, (e) => { loaded = "failed"; setError(reason(e)); });
    listActions().then(setActions, () => setError("Could not read the action list."));
  });
  const [pending, setPending] = createSignal(0);
  let queue: Promise<boolean> = Promise.resolve(true);
  /** Queues a change to the list and saves it. Input: the change, given the list as the previous update left it. Return: resolves whether it was stored. */
  const update = (change: (current: Rule[]) => Rule[]) => {
    if (loaded !== "ready") {
      if (loaded === "loading") setError(LOADING);
      return Promise.resolve(false);
    }
    const run = () => {
      const next = change(rules());
      return saveRules(next).then(
        () => { setRules(next); setError(""); return true; },
        (e) => { setError(String(e)); return false; },
      );
    };
    setPending((n) => n + 1);
    queue = queue.then(run, run);
    return queue.finally(() => setPending((n) => n - 1));
  };
  /** Whether an update is queued or saving, so a control that names a rule by its place can wait. */
  const busy = () => pending() > 0;
  return { rules, actions, error, update, busy };
}
