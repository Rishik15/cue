/**
 * Purpose: The pieces both preview layouts share: the data the window gets from Rust, a labelled scrolling block, the editable result box, the spinner and the footer.
 * Contents: Item / View — what `preview_get` returns; asView — checks that payload; closePreview — close that cannot fail; wordCount — "N words"; Block — a label, a clipped scrolling block and a note; Body — read-only text; ResultBox — the editable result;
 * Working — the spinner shown in a block; Footer — the Text label and the Paste or Cancel key.
 */
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LoaderCircle } from "lucide-solid";
import { createUniqueId, Show, type JSX } from "solid-js";
import { ActionHint } from "../ui/kit";

/** One palette row: `result` is the transformed text, null for a text-model row until it runs. */
export type Item = { id: string; name: string; hint: string; group: "captured" | "rule" | "clean" | "convert" | "extract" | "ai" | "snippet"; model: boolean; result: string | null };
export type View = { original: string; result: string | null; via: string; items: Item[] | null; mode: "paste" | "copy" | "snippets" };

const GROUPS = ["captured", "rule", "clean", "convert", "extract", "ai", "snippet"];
const isItem = (i: Item) =>
  typeof i?.id === "string" && typeof i.name === "string" && typeof i.hint === "string" && GROUPS.includes(i.group) && typeof i.model === "boolean" && (i.result === null || typeof i.result === "string");

/** Checks what `preview_get` returned before the page trusts it; null when it is missing or malformed. */
export function asView(v: View | null): View | null {
  const ok = v && typeof v.original === "string" && (v.result === null || typeof v.result === "string") && typeof v.via === "string" && ["paste", "copy", "snippets"].includes(v.mode);
  return ok && (v.items === null || (Array.isArray(v.items) && v.items.every(isItem))) ? v : null;
}

/** Closes the window through Rust (which also rescues dictated text); if that call fails the window is closed directly, so it can never be stuck open. */
export const closePreview = (text?: string) =>
  invoke("preview_close", { text: text ?? null }).catch(() => getCurrentWindow().close().catch(() => undefined));

/** A spellchecker over a very long text costs more than it helps. */
const SPELLCHECK_MAX = 20_000;

export const wordCount = (text: string) => {
  const n = text.trim().split(/\s+/).filter(Boolean).length;
  return `${n} ${n === 1 ? "word" : "words"}`;
};

/** A label over a block, with a one-line note under it (the reference form's helper text). The block's size is the column's, never the text's. */
export function Block(p: { label: string; note?: string; muted?: boolean; bare?: boolean; children?: JSX.Element }) {
  const heading = createUniqueId();
  return (
    <section class="flex min-h-0 min-w-0 flex-1 flex-col gap-2">
      <h2 id={heading} class="t-label [font-weight:var(--w-strong)]">{p.label}</h2>
      {/* The frame clips; the scroller inside it carries the padding, so text never runs under the border and the scrollbar sits inside the frame. */}
      <div class="t-label min-h-0 flex-1 overflow-hidden rounded-xl border border-line-strong bg-field select-text" classList={{ "!text-ink/75": p.muted }}>
        {/* A scrolling region must be reachable by keyboard, so it takes focus (arrow keys scroll it) and is named by its label. */}
        <div class="h-full" classList={{ "thin-scroll fade-edges overflow-auto px-3 py-2.5": !p.bare }} tabindex={p.bare ? undefined : 0} role={p.bare ? undefined : "region"} aria-labelledby={heading}>{p.children}</div>
      </div>
      <p class="t-hint min-h-4 px-0.5">{p.note}</p>
    </section>
  );
}

export const Body = (p: { text: string }) => <pre class="whitespace-pre-wrap break-words font-[inherit] leading-5">{p.text || "Empty"}</pre>;

/** The result as a text box: the user may fix it before it is pasted. `focus` gives it the keyboard when it appears (the dictation preview has no other input). */
export function ResultBox(p: { value: string; onInput: (v: string) => void; focus?: boolean }) {
  return (
    <textarea
      ref={(el) => p.focus && queueMicrotask(() => el.focus())}
      aria-label="Result"
      value={p.value}
      spellcheck={p.value.length < SPELLCHECK_MAX}
      onInput={(e) => p.onInput(e.currentTarget.value)}
      class="thin-scroll fade-edges block h-full w-full resize-none overflow-auto bg-transparent px-3 py-2.5 leading-5 outline-none"
    />
  );
}

export const Working = (p: { label: string }) => (
  <div class="flex h-full items-center justify-center gap-2 text-muted" role="status">
    <LoaderCircle size={16} class="spin" />
    <span class="t-label !text-muted">{p.label}</span>
  </div>
);

/** Text icon and label at the left; at the right Paste when there is something to paste, otherwise Cancel. */
export function Footer(p: { onPaste?: () => void; onCancel: () => void; verb?: string }) {
  return (
    <footer class="flex h-10 shrink-0 items-center justify-between gap-4 border-t border-line pl-5 pr-4">
      <span class="t-label">Text</span>
      <Show when={p.onPaste} fallback={<ActionHint label="Cancel" keys={["Esc"]} onClick={p.onCancel} />}>
        {(paste) => <ActionHint label={p.verb ?? "Paste"} keys={["Ctrl", "↵"]} onClick={paste()} />}
      </Show>
    </footer>
  );
}
