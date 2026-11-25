/**
 * File: useRules.ts
 * Purpose: State behind the Rules page: the saved rules, the action catalogue, and saving with the reason when Rust refuses.
 * Contents: useRules — signals plus update (change the list; resolves true when stored; refused until the saved rules have been read and for good when they could not be,
 * so a save never replaces a file Cue has not loaded). Updates run one after another, each on the list as the
 * previous one left it, so switching two rules quickly cannot undo the first.
 */
import { createSignal, onMount } from "solid-js";
import { getRules, listActions, saveRules, type ActionInfo, type Rule } from "./rules";

/** The Rules page state. Return: the rules, the action catalogue, the last error and `update`, which applies a change to the list and resolves whether it was stored. */
export function useRules() {
  const [rules, setRules] = createSignal<Rule[]>([]);
  const [actions, setActions] = createSignal<ActionInfo[]>([]);
  const [error, setError] = createSignal("");
  let loaded: "loading" | "ready" | "failed" = "loading";
  onMount(() => {
    getRules().then((r) => { setRules(r); loaded = "ready"; }, (e) => { loaded = "failed"; setError(String(e)); });
    listActions().then(setActions, () => setError("Could not read the action list."));
  });
  let queue: Promise<boolean> = Promise.resolve(true);
  const update = (change: (current: Rule[]) => Rule[]) => {
    if (loaded !== "ready") return Promise.resolve(false);
    const run = () => {
      const next = change(rules());
      return saveRules(next).then(
        () => { setRules(next); setError(""); return true; },
        (e) => { setError(String(e)); return false; },
      );
    };
    queue = queue.then(run, run);
    return queue;
  };
  return { rules, actions, error, update };
}
