/**
 * Purpose: The preview window: the text before and after a rule changed it, with Apply as the one main action, plus Copy and Cancel.
 * Contents: Preview — the window body; Block — one labelled, scrollable text; useView — the pending text from Rust, refreshed when it changes.
 * Enter applies and Escape cancels.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { X } from "lucide-solid";
import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { Button } from "../ui/kit";

type View = { original: string; result: string };

function useView() {
  const [view, setView] = createSignal<View | null>(null);
  const load = () => invoke<View | null>("preview_get").then(setView, () => setView(null));
  onMount(() => {
    load();
    const unlisten = listen("preview-changed", load);
    onCleanup(() => unlisten.then((f) => f()));
  });
  return view;
}

function Block(p: { label: string; text: string; muted?: boolean }) {
  return (
    <section class="flex min-h-0 flex-1 flex-col gap-1">
      <h2 class="t-label px-1">{p.label}</h2>
      <pre
        class="t-control min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-card px-3 py-2 font-[inherit] select-text"
        classList={{ "!text-muted": p.muted }}
      >
        {p.text || "Empty"}
      </pre>
    </section>
  );
}

export function Preview() {
  const view = useView();
  const act = (command: string) => invoke(command);
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Escape") act("preview_close");
    if (e.key === "Enter") act("preview_apply");
  };
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return (
    <div class="flex h-full flex-col bg-bg">
      <header data-tauri-drag-region class="flex h-10 shrink-0 items-center justify-between border-b border-line pl-4">
        <span data-tauri-drag-region class="t-label">Review Changes</span>
        <button aria-label="Close" onClick={() => getCurrentWindow().close()} class="grid h-[39px] w-[46px] place-items-center text-muted hover:bg-danger hover:text-white">
          <X size={15} />
        </button>
      </header>
      <Show when={view()} fallback={<p class="t-hint p-4">Nothing to review.</p>}>
        {(v) => (
          <main class="flex min-h-0 flex-1 flex-col gap-3 p-4">
            <Block label="Original" text={v().original} muted />
            <Block label="Result" text={v().result} />
          </main>
        )}
      </Show>
      <footer class="flex h-12 shrink-0 items-center justify-between border-t border-line px-4">
        <span class="t-micro">Enter applies · Esc cancels</span>
        <div class="flex gap-2">
          <Button onClick={() => act("preview_close")}>Cancel</Button>
          <Button onClick={() => act("preview_copy")}>Copy</Button>
          <Button primary onClick={() => act("preview_apply")}>Apply</Button>
        </div>
      </footer>
    </div>
  );
}
