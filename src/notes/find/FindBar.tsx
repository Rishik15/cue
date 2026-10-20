/**
 * Purpose: The find and replace bar drawn at the top of the note editor.
 * Contents: FindBar — the Solid component (find row, replace row, keys); findPanel — mounts it as a CodeMirror panel.
 */
import type { EditorView, Panel } from "@codemirror/view";
import { Show } from "solid-js";
import { render } from "solid-js/web";
import { createFind, registerFind, type Find } from "./useFind";

const input = "t-label h-6 min-w-0 flex-1 rounded-md bg-field px-2";
const text = "t-hint h-6 shrink-0 rounded-md px-2 hover:bg-hover hover:text-ink";

const act = (e: KeyboardEvent, run: () => void) => {
  e.preventDefault();
  run();
};

function findAction(f: Find, e: KeyboardEvent) {
  if (e.key === "ArrowDown" || (e.key === "Enter" && !e.shiftKey)) return f.next;
  if (e.key === "ArrowUp" || e.key === "Enter") return f.prev;
}

// Keys inside the bar stay inside it: none reach the editor's keymaps.
function keys(f: Find, second: boolean) {
  return (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Escape") return act(e, f.close);
    if (e.key === "Tab" && e.shiftKey === second) return act(e, () => f.focus(!second));
    const run = second ? (e.key === "Enter" ? (e.shiftKey ? f.replaceEvery : f.replaceOne) : undefined) : findAction(f, e);
    if (run) act(e, run);
  };
}
function Count(f: Find) {
  return (
    <Show when={f.search()}>
      <span class="t-hint tabular-nums" classList={{ "!text-danger": !f.count()[1] }}>{f.count()[1] ? `${f.count()[0]}/${f.count()[1]}` : "0"}</span>
    </Show>
  );
}

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
