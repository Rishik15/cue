/**
 * Purpose: The preview window's page. After dictation it shows a finished result for review, laid out like a launcher form on an acrylic backdrop: the action's name and the Original text on the
 * left and, on the right, the Result in an editable text box (what is in the box is what gets pasted). For a selection it hands over to the palette (`Palette`).
 * It is built hidden at the press and shown once the text is copied (never when nothing is selected; a 2 s fallback in `index.tsx` shows the "Reading…" frame if copying is slow). Ctrl+Enter pastes, Escape or Cancel closes.
 * Contents: Preview — the window; useView — the pending text from Rust, refreshed when it changes, which also shows the window once there is text; DictationPreview — the review layout.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createEffect, createSignal, onCleanup, onMount, Show } from "solid-js";
import { ValueField } from "../ui/kit";
import { Palette, PaletteLoading } from "./Palette";
import { asView, Block, Body, closePreview, Footer, ResultBox, Working, wordCount, type View } from "./parts";

function useView() {
  const [view, setView] = createSignal<View | null>(null);
  // Shown only once there is something to show: when nothing is selected the window never appears and only the notice does.
  const load = () =>
    invoke<View | null>("preview_get").then(
      (raw) => {
        const v = asView(raw);
        setView(v);
        if (v) getCurrentWindow().show().catch(() => undefined);
      },
      () => setView(null),
    );
  onMount(() => {
    // The listener is registered before the first read: an event sent in between would otherwise be lost and the page would wait for ever.
    let off: (() => void) | undefined;
    let gone = false;
    listen("preview-changed", load).then(
      (f) => {
        if (gone) f();
        else off = f;
        void load();
      },
      () => void load(),
    );
    onCleanup(() => {
      gone = true;
      off?.();
    });
  });
  return view;
}

/** The action's name over the Original text, stacked in the left column; the Original block ends level with the Result block. */
function Original(p: { view: View | null }) {
  return (
    <div class="flex min-h-0 min-w-0 flex-col gap-[9px]">
      <ValueField label="Action" value={p.view?.via ?? ""} />
      <Block label="Original" note={p.view ? wordCount(p.view.original) : undefined} muted>
        <Show when={p.view} fallback={<Working label="Reading the selection" />}>{(v) => <Body text={v().original} />}</Show>
      </Block>
    </div>
  );
}

function DictationPreview(p: { view: View | null }) {
  const [draft, setDraft] = createSignal("");
  createEffect(() => setDraft(p.view?.result ?? ""));
  const ready = () => p.view?.result != null;
  const paste = () => invoke("preview_apply", { text: draft() }).catch(() => closePreview(draft()));
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Escape") void closePreview(draft());
    // Enter is a new line inside the result box, so pasting from there takes Ctrl+Enter.
    if (e.key === "Enter" && (e.ctrlKey || e.target === document.body)) void paste();
  };
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return (
    <>
      <header data-tauri-drag-region class="h-6 shrink-0" />
      <main class="grid min-h-0 flex-1 grid-cols-2 gap-6 px-14 pb-3">
        <Original view={p.view} />
        <Block label="Result" note={ready() ? wordCount(draft()) : undefined} bare={ready()}>
          <Show when={ready()} fallback={<Working label="Reading the selection" />}>
            <ResultBox value={draft()} onInput={setDraft} focus />
          </Show>
        </Block>
      </main>
      <Footer onPaste={ready() ? paste : undefined} onCancel={() => closePreview(draft())} />
    </>
  );
}

export function Preview() {
  const view = useView();
  return (
    <div class="flex h-full flex-col bg-card/70">
      <Show when={view()} fallback={<PaletteLoading />}>
        <Show when={view()?.items} fallback={<DictationPreview view={view()} />}>{(items) => <Palette original={view()?.original ?? ""} items={items()} mode={view()?.mode ?? "paste"} />}</Show>
      </Show>
    </div>
  );
}
