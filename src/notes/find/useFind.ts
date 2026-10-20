/**
 * Purpose: State and actions of the find bar: the query fields, the match count, and the calls into CodeMirror's search engine.
 * Contents: createFind — signals plus next/prev/replace/all/close; findFor — the open bar of a view (for Ctrl+F and Ctrl+H).
 */
import type { EditorView, ViewUpdate } from "@codemirror/view";
import { closeSearchPanel, findNext, findPrevious, getSearchQuery, replaceAll, replaceNext, SearchQuery, setSearchQuery } from "@codemirror/search";
import { createSignal } from "solid-js";

const COUNT_CAP = 999;
const bars = new WeakMap<EditorView, Find>();
export type Find = ReturnType<typeof createFind>;
export const findFor = (view: EditorView) => bars.get(view);
export const registerFind = (view: EditorView, f: Find | undefined) => (f ? bars.set(view, f) : bars.delete(view));

// [index of the match at or after the cursor, total], counting no further than the cap so a huge note stays cheap.
function tally(view: EditorView, q: SearchQuery): [number, number] {
  if (!q.valid) return [0, 0];
  const head = view.state.selection.main.from;
  const cur = q.getCursor(view.state);
  let at = 0;
  let total = 0;
  for (let r = cur.next(); !r.done && total < COUNT_CAP; r = cur.next()) {
    total++;
    if (!at && r.value.from >= head) at = total;
  }
  return [at || Math.min(1, total), total];
}

export function createFind(view: EditorView) {
  const start = getSearchQuery(view.state);
  const [search, setSearch] = createSignal(start.search);
  const [replace, setReplace] = createSignal(start.replace);
  const [matchCase, setMatchCase] = createSignal(start.caseSensitive);
  const [replacing, setReplacing] = createSignal(false);
  const [count, setCount] = createSignal<[number, number]>([0, 0]);
  const refs: { find?: HTMLInputElement; replace?: HTMLInputElement } = {};

  const query = () => new SearchQuery({ search: search(), caseSensitive: matchCase(), replace: replace() });
  const push = () => view.dispatch({ effects: setSearchQuery.of(query()) });
  const focus = (second: boolean) => {
    if (second) setReplacing(true);
    const el = second ? refs.replace : refs.find;
    el?.focus();
    el?.select();
  };
  // Typing selects the first match from the cursor on, so the note shows what the count counts.
  const edit = (set: () => void, jump: boolean) => {
    set();
    push();
    const hit = jump && query().valid ? query().getCursor(view.state, view.state.selection.main.from).next() : undefined;
    if (hit && !hit.done) view.dispatch({ selection: { anchor: hit.value.from, head: hit.value.to }, scrollIntoView: true });
  };
  const sync = (u: ViewUpdate) => {
    if (!u.transactions.length) return;
    const q = getSearchQuery(u.state);
    setSearch(q.search);
    setReplace(q.replace);
    setMatchCase(q.caseSensitive);
    setCount(tally(view, q));
  };
  const close = () => {
    closeSearchPanel(view);
    view.focus();
  };

  const toggleCase = () => edit(() => setMatchCase((c) => !c), true);

  return {
    search, replace, matchCase, replacing, count, refs, focus, sync, close,
    setSearch: (v: string) => edit(() => setSearch(v), true),
    setReplace: (v: string) => edit(() => setReplace(v), false),
    toggleCase,
    next: () => findNext(view),
    prev: () => findPrevious(view),
    replaceOne: () => replaceNext(view),
    replaceEvery: () => replaceAll(view),
  };
}
