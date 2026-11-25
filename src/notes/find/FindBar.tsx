/**
 * File: notes/find/FindBar.tsx
 * Purpose: The find and replace bar drawn at the top of the note editor.
 * Contents: FindBar — the Solid component (find row, replace row, keys); findPanel — mounts it as a CodeMirror panel.
 */
import type { EditorView, Panel } from "@codemirror/view";
import { Show } from "solid-js";
import { render } from "solid-js/web";
import { composing } from "../../ui/keys";
import { createFind, registerFind, type Find } from "./useFind";

const input = "t-label h-6 min-w-0 flex-1 rounded-md bg-field px-2";
const text = "t-hint h-6 shrink-0 rounded-md px-2 hover:bg-hover hover:text-ink";

/** Takes a key from the browser and runs its action. Inputs: the key event and the action. */
const act = (e: KeyboardEvent, run: () => void) => {
  e.preventDefault();
  run();
};

/** The find row's action for a key: Down or Enter goes to the next match, Up or Shift+Enter to the previous. Inputs: the find state and the key event. Return: the action, or undefined. */
function findAction(f: Find, e: KeyboardEvent) {
  if (e.key === "ArrowDown" || (e.key === "Enter" && !e.shiftKey)) return f.next;
  if (e.key === "ArrowUp" || e.key === "Enter") return f.prev;
}

/** The key handler of one row of the bar; keys stay inside the bar (none reach the editor's keymaps) and keys of an input method are its own. Inputs: the find state and whether this is the replace row. Return: the handler. */
function keys(f: Find, second: boolean) {
  return (e: KeyboardEvent) => {
    e.stopPropagation();
    if (composing(e)) return;
    if (e.key === "Escape") return act(e, f.close);
    if (e.key === "Tab" && e.shiftKey === second) return act(e, () => f.focus(!second));
    const run = second ? (e.key === "Enter" ? (e.shiftKey ? f.replaceEvery : f.replaceOne) : undefined) : findAction(f, e);
    if (run) act(e, run);
  };
}
/** The match count ("n/total", or a red 0) while there is a search. Input: the find state. Return: the count. */
function Count(f: Find) {
  return (
    <Show when={f.search()}>
      <span class="t-hint tabular-nums" classList={{ "!text-danger": !f.count()[1] }}>{f.count()[1] ? `${f.count()[0]}/${f.count()[1]}` : "0"}</span>
    </Show>
  );
}

/** The bar: the find row and, when open, the replace row. Input: the find state. Return: the bar. */
export function FindBar(f: Find) {
  return (
    <div class="flex flex-col py-0.5">
      <div class="flex h-8 items-center gap-2 px-10">
        <input ref={(el) => (f.refs.find = el)} main-field="true" value={f.search()} onInput={(e) => f.setSearch(e.currentTarget.value)} onKeyDown={keys(f, false)} placeholder="Find" aria-label="Find" class={input} />
        {Count(f)}
        <button tabindex="-1" onClick={() => f.toggleCase()} aria-label="Match case" aria-pressed={f.matchCase()} class={text} classList={{ "!text-accent": f.matchCase() }}>Aa</button>
      </div>
      <Show when={f.replacing()}>
        <div class="flex h-8 items-center gap-2 px-10">
          <input ref={(el) => (f.refs.replace = el)} value={f.replace()} onInput={(e) => f.setReplace(e.currentTarget.value)} onKeyDown={keys(f, true)} placeholder="Replace" aria-label="Replace with" class={input} />
          <button tabindex="-1" onClick={() => f.replaceOne()} aria-label="Replace" class={text}>Replace</button>
          <button tabindex="-1" onClick={() => f.replaceEvery()} aria-label="Replace all" class={text}>All</button>
        </div>
      </Show>
    </div>
  );
}

/** Mounts the bar as a CodeMirror panel over the editor. Input: the editor view. Return: the panel. */
export function findPanel(view: EditorView): Panel {
  const dom = document.createElement("div");
  dom.className = "find-bar";
  const f = createFind(view);
  const dispose = render(() => FindBar(f), dom);
  registerFind(view, f);
  return {
    dom,
    top: true,
    mount: () => f.focus(false),
    update: f.sync,
    destroy: () => {
      registerFind(view, undefined);
      dispose();
    },
  };
}
