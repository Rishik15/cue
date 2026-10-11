/**
 * Purpose: The preview window, laid out like a Raycast form: a back button, the text before and after side by side, and a footer of text
 * actions with their keys. Enter applies, Ctrl+Enter copies the result, Escape or the back arrow cancels.
 * Contents: Preview — the window; Block — one labelled, scrollable text; useView — the pending text from Rust, refreshed when it changes.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ArrowLeft, WandSparkles } from "lucide-solid";
import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { ActionHint, IconButton } from "../ui/kit";

type View = { original: string; result: string; via: string };

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
    <section class="flex min-h-0 min-w-0 flex-col gap-2">
      <h2 class="t-hint px-0.5">{p.label}</h2>
      <pre
        class="t-control min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-line bg-card px-3 py-2 font-[inherit] select-text"
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
    if (e.key === "Enter") act(e.ctrlKey ? "preview_copy" : "preview_apply");
  };
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return (
    <div class="flex h-full flex-col bg-bg">
      <header data-tauri-drag-region class="flex h-12 shrink-0 items-center justify-between gap-4 px-3">
        <IconButton label="Cancel" onClick={() => act("preview_close")}><ArrowLeft size={16} /></IconButton>
        <span data-tauri-drag-region class="t-hint min-w-0 truncate" title={view()?.via}>{view()?.via}</span>
      </header>
      <Show when={view()} fallback={<p class="t-hint flex-1 px-4">Nothing to review.</p>}>
        {(v) => (
          <main class="grid min-h-0 flex-1 grid-cols-2 gap-4 px-4 pb-4">
            <Block label="Original" text={v().original} muted />
            <Block label="Result" text={v().result} />
          </main>
        )}
      </Show>
      <footer class="flex h-11 shrink-0 items-center justify-between border-t border-line px-3">
        <span class="t-label flex items-center gap-2"><WandSparkles size={16} class="text-muted" />Review Changes</span>
        <div class="flex items-center gap-1">
          <ActionHint label="Copy" keys={["Ctrl", "↵"]} onClick={() => act("preview_copy")} muted />
          <ActionHint label="Apply" keys={["↵"]} onClick={() => act("preview_apply")} />
        </div>
      </footer>
    </div>
  );
}
