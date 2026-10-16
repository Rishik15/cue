/**
 * Purpose: The dictionary popup's page: the selected word as a dictionary entry (word, pronunciation, part of speech, meanings with examples, synonyms) and a footer to save it.
 * It measures itself once drawn and asks Rust to size, place and show the window beside the pointer. Escape closes; so does clicking away (Rust).
 * Contents: Define — the window; closeDefine — close without doing anything else.
 */
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { X } from "lucide-solid";
import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { toDefinition, type Definition } from "../dictionary";
import { ActionHint } from "../ui/kit";
import { WordBlock } from "../ui/WordBlock";

export const closeDefine = () => invoke("define_close").catch(() => getCurrentWindow().close().catch(() => undefined));

function Footer(p: { language: string; saved: boolean; onSave: () => void }) {
  return (
    <footer class="flex h-10 shrink-0 items-center justify-between border-t border-line pl-5 pr-[11px]">
      <span class="t-hint">{p.language}</span>
      <Show when={!p.saved} fallback={<span class="t-label px-2 text-muted">Saved</span>}>
        <ActionHint label="Save Word" keys={["Ctrl", "S"]} onClick={p.onSave} />
      </Show>
    </footer>
  );
}

export function Define() {
  const [entry, setEntry] = createSignal<Definition | null>(null);
  const [box, setBox] = createSignal<HTMLDivElement>();
  const save = () => void invoke("define_save").then(() => setEntry((d) => d && { ...d, saved: true }), () => undefined);
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Escape") void closeDefine();
    if (e.key === "s" && e.ctrlKey && entry() && !entry()?.saved) save();
  };
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
    // Drawn first, measured next, then Rust places and shows the window at that height.
    void invoke("define_get").then((v) => {
      const d = toDefinition(v);
      if (!d) return void closeDefine();
      setEntry(d);
      requestAnimationFrame(() => void invoke("define_fit", { height: box()?.offsetHeight ?? 200 }).catch(() => closeDefine()));
    }, () => closeDefine());
  });
  return (
    <div ref={setBox} class="flex max-h-[520px] flex-col bg-card">
      <header data-tauri-drag-region class="relative h-6 shrink-0">
        <button onClick={() => void closeDefine()} aria-label="Close" class="absolute right-2.5 top-1.5 grid size-5 place-items-center text-muted"><X size={14} /></button>
      </header>
      <Show when={entry()}>
        {(d) => (
          <>
            <main class="thin-scroll flex min-h-0 flex-col gap-4 overflow-auto px-5 pb-4 pt-1">
              <For each={d().blocks}>
                {(block, i) => (
                  <WordBlock block={block} language={d().language} head={i() === 0 || block.word !== d().blocks[i() - 1]?.word} />
                )}
              </For>
            </main>
            <Footer language={d().language} saved={d().saved} onSave={save} />
          </>
        )}
      </Show>
    </div>
  );
}
