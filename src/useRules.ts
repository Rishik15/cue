/**
 * Purpose: State behind the Rules page: the saved rules, the action catalogue, and saving with the reason when Rust refuses.
 * Contents: useRules — signals plus save (replace the whole list; resolves true when stored).
 */
import { createSignal, onMount } from "solid-js";
import { getRules, listActions, saveRules, type ActionInfo, type Rule } from "./rules";

export function useRules() {
  const [rules, setRules] = createSignal<Rule[]>([]);
  const [actions, setActions] = createSignal<ActionInfo[]>([]);
  const [error, setError] = createSignal("");
  onMount(() => {
    getRules().then(setRules, (e) => setError(String(e)));
    listActions().then(setActions, () => setError("Could not read the action list."));
  });
  const save = (next: Rule[]) =>
    saveRules(next).then(
      () => { setRules(next); setError(""); return true; },
      (e) => { setError(String(e)); return false; },
    );
  return { rules, actions, error, save };
}
