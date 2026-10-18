/**
 * Purpose: State behind the Rules page: the saved rules, the action catalogue, and saving with the reason when Rust refuses.
 * Contents: useRules — signals plus update (change the list; resolves true when stored; refused while the saved rules could not be read, so a save never replaces a file Cue could not load). Updates run one after another, each on the list as the
 * previous one left it, so switching two rules quickly cannot undo the first.
 */
import { createSignal, onMount } from "solid-js";
import { getRules, listActions, saveRules, type ActionInfo, type Rule } from "./rules";

export function useRules() {
  const [rules, setRules] = createSignal<Rule[]>([]);
  const [actions, setActions] = createSignal<ActionInfo[]>([]);
  const [error, setError] = createSignal("");
  let unreadable = false;
  onMount(() => {
    getRules().then(setRules, (e) => { unreadable = true; setError(String(e)); });
    listActions().then(setActions, () => setError("Could not read the action list."));
  });
  let queue: Promise<boolean> = Promise.resolve(true);
  const update = (change: (current: Rule[]) => Rule[]) => {
    if (unreadable) return Promise.resolve(false);
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
