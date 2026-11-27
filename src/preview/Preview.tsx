/**
 * File: preview/Preview.tsx
 * Purpose: The preview window's page. After dictation it shows a finished result for review, laid out like a launcher form on an acrylic backdrop: the action's name and the Original text on the
 * left and, on the right, the Result in an editable text box (what is in the box is what gets pasted). For a screen grab it hands over to the palette (`Palette`).
 * It is built hidden at the press and shown by Rust once the text is copied (never when nothing is selected). Ctrl+Enter pastes, Escape or Cancel closes.
 * Contents: Preview — the window; useView — the pending text from Rust, refreshed when it changes; DictationPreview — the review layout.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createEffect, createSignal, onCleanup, onMount, Show } from "solid-js";
import { ValueField } from "../ui/kit";
import { composing } from "../ui/keys";
import { Palette, PaletteLoading } from "./Palette";
import { asView, Block, Body, closePreview, applyPreview, Footer, ResultBox, Working, wordCount, type View } from "./parts";

/** The pending text from Rust, read again whenever Rust says it changed. The listener is registered before the first read, so an event sent in between is never lost. Return: the view signal. */
function useView() {
  const [view, setView] = createSignal<View | null>(null);
  /** Reads the pending text; reading it also ends Rust's watchdog of this window. */
  const load = () => invoke("preview_get").then((raw) => setView(asView(raw)), () => setView(null));
  onMount(() => {
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

/** The review layout after dictation: Original at the left, the editable Result at the right (what is in the box is pasted), the footer below. Input: the view. */
function DictationPreview(p: { view: View | null }) {
  const [draft, setDraft] = createSignal("");
  createEffect(() => setDraft(p.view?.result ?? ""));
  /** Whether the result has arrived. */
  const ready = () => p.view?.result != null;
  /** Closes this result's preview with the box's text. */
  const close = () => closePreview(p.view?.session ?? 0, draft());
  /** Pastes the box's text; if Rust refuses, the window closes, rescuing the text. */
  const paste = () => applyPreview(p.view?.session ?? 0, draft());
  /** Escape closes; Enter pastes from outside the box, Ctrl+Enter from inside it (where Enter is a new line); keys of an input method are its own. Input: the key event. */
  const keys = (e: KeyboardEvent) => {
    if (composing(e)) return;
    if (e.key === "Escape") void close();
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
      <Footer onPaste={ready() ? paste : undefined} onCancel={close} />
    </>
  );
}

/** The window: the loading frame until Rust has text, then the palette for a screen grab or the review layout after dictation. */
export function Preview() {
  const view = useView();
  return (
    <div class="flex h-full flex-col bg-card/70">
      <Show when={view()} fallback={<PaletteLoading />}>
        <Show when={view()?.items} fallback={<DictationPreview view={view()} />}>{(items) => <Palette original={view()?.original ?? ""} items={items()} mode={view()?.mode ?? "paste"} session={view()?.session ?? 0} />}</Show>
      </Show>
    </div>
  );
}
