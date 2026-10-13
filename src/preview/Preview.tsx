/**
 * Purpose: The preview window, shaped like a launcher form on an acrylic backdrop: a back button, the text before and after side by side,
 * and a footer with the action's name and its keys. It opens the moment the text is copied and shows a working state in the result column until
 * the actions finish. Enter applies, Ctrl+Enter copies, Escape or the back arrow cancels (and stops the work).
 * With several matching actions it first lists them (Up and Down, Enter runs the highlighted one) instead of the result.
 * Contents: Preview — the window; Picker — the list of actions to choose from; Block — one labelled, scrollable text; Working — the result column while the actions run; Footer — the action
 * name and keys; useView — the pending text from Rust, refreshed when it changes, which also shows the window once there is something to show.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LoaderCircle } from "lucide-solid";
import { createEffect, createSignal, For, onCleanup, onMount, Show, type JSX } from "solid-js";
import { ActionDivider, ActionHint, BackButton } from "../ui/kit";
import { NavIcon } from "../ui/NavIcon";
import { Text } from "../ui/glyphs";

type Choice = { at: number; name: string; steps: string };
type View = { original: string; result: string | null; via: string; choices: Choice[] | null };

function useView() {
  const [view, setView] = createSignal<View | null>(null);
  const load = () => invoke<View | null>("preview_get").then(setView, () => setView(null));
  onMount(() => {
    load();
    const unlisten = listen("preview-changed", load);
    onCleanup(() => unlisten.then((f) => f()));
  });
  // The window is created hidden, so an empty one is never seen: it appears when Rust has the text.
  let shown = false;
  createEffect(() => {
    if (!view() || shown) return;
    shown = true;
    const win = getCurrentWindow();
    win.show().then(() => win.setFocus());
  });
  return view;
}

function Block(p: { label: string; muted?: boolean; children: JSX.Element }) {
  return (
    <section class="flex min-h-0 min-w-0 flex-col gap-2">
      <h2 class="t-meta px-1">{p.label}</h2>
      <div
        class="t-label min-h-0 flex-1 overflow-auto rounded-xl border border-line-strong bg-field px-4 py-3 select-text"
        classList={{ "!text-muted": p.muted }}
      >
        {p.children}
      </div>
    </section>
  );
}

function Picker(p: { choices: Choice[]; active: number; onPick: (at: number) => void }) {
  return (
    <Block label="Choose an Action">
      <div class="-mx-2 -my-1 flex flex-col gap-px" role="listbox" aria-label="Actions">
        <For each={p.choices}>
          {(c, i) => (
            <button
              role="option"
              aria-selected={i() === p.active}
              onClick={() => p.onPick(c.at)}
              class="flex min-h-9 w-full min-w-0 flex-col items-start justify-center rounded-lg px-2 py-1 text-left"
              classList={{ "bg-selected": i() === p.active }}
              title={c.name}
            >
              <span class="t-label w-full truncate">{c.name}</span>
              <span class="t-hint w-full truncate">{c.steps}</span>
            </button>
          )}
        </For>
      </div>
    </Block>
  );
}

const Working = () => (
  <div class="flex h-full items-center justify-center gap-2 text-muted" role="status">
    <LoaderCircle size={16} class="spin" />
    <span class="t-label !text-muted">Working on it…</span>
  </div>
);

function Footer(p: { via: string; ready: boolean; choosing: boolean; act: (command: string) => void; run: () => void }) {
  return (
    <footer class="flex h-14 shrink-0 items-center justify-between gap-4 border-t border-line px-6">
      <span class="flex min-w-0 items-center gap-2.5" title={p.via}>
        <NavIcon icon={Text} size={24} />
        <span class="t-label truncate">{p.choosing ? "Choose an action" : p.via}</span>
      </span>
      <div class="flex shrink-0 items-center">
        <Show when={p.choosing}>
          <ActionHint label="Run" keys={["↵"]} onClick={p.run} />
          <ActionDivider />
          <ActionHint label="Cancel" keys={["Esc"]} onClick={() => p.act("preview_close")} muted />
        </Show>
        <Show when={!p.choosing}>
          <Show when={p.ready} fallback={<ActionHint label="Cancel" keys={["Esc"]} onClick={() => p.act("preview_close")} />}>
            <ActionHint label="Apply" keys={["↵"]} onClick={() => p.act("preview_apply")} />
            <ActionDivider />
            <ActionHint label="Copy" keys={["Ctrl", "↵"]} onClick={() => p.act("preview_copy")} muted />
          </Show>
        </Show>
      </div>
    </footer>
  );
}

export function Preview() {
  const view = useView();
  const [active, setActive] = createSignal(0);
  const act = (command: string) => invoke(command);
  const choices = () => view()?.choices ?? null;
  const pick = (at: number) => invoke("preview_choose", { at });
  const run = () => { const c = choices()?.[active()]; if (c) pick(c.at); };
  const move = (by: number) => setActive(Math.max(0, Math.min((choices()?.length ?? 1) - 1, active() + by)));
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Escape") act("preview_close");
    if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); move(e.key === "ArrowDown" ? 1 : -1); }
    if (e.key !== "Enter") return;
    if (choices()) run(); else act(e.ctrlKey ? "preview_copy" : "preview_apply");
  };
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return (
    <div class="flex h-full flex-col bg-card/70">
      <header data-tauri-drag-region class="flex h-14 shrink-0 items-center px-6">
        <BackButton onClick={() => act("preview_close")} />
      </header>
      <Show when={view()}>
        {(v) => (
          <>
            <main class="grid min-h-0 flex-1 grid-cols-2 gap-6 px-6 pb-6">
              <Block label="Original" muted><pre class="whitespace-pre-wrap break-words font-[inherit]">{v().original || "Empty"}</pre></Block>
              <Show
                when={v().choices}
                fallback={
                  <Block label="Result">
                    <Show when={v().result !== null} fallback={<Working />}>
                      <pre class="whitespace-pre-wrap break-words font-[inherit]">{v().result || "Empty"}</pre>
                    </Show>
                  </Block>
                }
              >
                {(list) => <Picker choices={list()} active={active()} onPick={pick} />}
              </Show>
            </main>
            <Footer via={v().via} ready={v().result !== null} choosing={!!v().choices} act={act} run={run} />
          </>
        )}
      </Show>
    </div>
  );
}
