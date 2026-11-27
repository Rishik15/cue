/**
 * File: preview/parts.tsx
 * Purpose: The pieces both preview layouts share: the data the window gets from Rust, a labelled scrolling block, the editable result box, the spinner and the footer.
 * Contents: Item / View — what `preview_get` returns; GROUPS / isItem / asView — checks that payload; closePreview — close one session's preview; applyPreview — paste one session's text, closing that session if Rust refuses; wordCount — "N words"; Block — a label, a clipped scrolling block and a note; Body — read-only text; ResultBox — the editable result;
 * Working — five bars in a wave (no text), shown in a block while something loads; Footer — the shared ActionFooter with the Text label and the Paste or Cancel action.
 */
import { invoke } from "@tauri-apps/api/core";
import { createUniqueId, For, type JSX } from "solid-js";
import { isBool, isStr, listOf, nullable, shaped } from "../ipc";
import { ActionFooter } from "../ui/kit";

/** One palette row: `result` is the transformed text, null for a text-model row until it runs. */
export type Item = { id: string; name: string; hint: string; group: "captured" | "rule" | "clean" | "convert" | "extract" | "ai"; model: boolean; result: string | null };
/** What `preview_get` returns: the original, the result (null until it arrives), the action's name, the palette rows (null for dictation), whether Enter pastes or copies, and the palette session. */
export type View = { original: string; result: string | null; via: string; items: Item[] | null; mode: "paste" | "copy"; session: number };

/** The palette groups a row may belong to. */
const GROUPS = ["captured", "rule", "clean", "convert", "extract", "ai"];
/** Whether a value is one row with the right types and a known group. Input: the value. Return: the check. */
const isItem = (i: unknown): i is Item =>
  shaped<Item>(i, { id: isStr, name: isStr, hint: isStr, group: (g) => GROUPS.some((x) => x === g), model: isBool, result: nullable(isStr) });

/** Checks what `preview_get` returned before the page trusts it; the palette session must be a whole number. Input: the reply. Return: the view, or null when it is missing or malformed. */
export const asView = (v: unknown): View | null =>
  shaped<View>(v, {
    original: isStr, result: nullable(isStr), via: isStr, items: nullable(listOf(isItem)), mode: (m) => m === "paste" || m === "copy", session: Number.isSafeInteger,
  }) ? v : null;

/** Closes the window through Rust (which also rescues dictated text); Rust ignores a close naming a session that is no longer the window's, so a stale page never closes newer text. Input: the session the page shows and the box's text. */
export const closePreview = (session: number, text?: string) => invoke("preview_close", { session, text: text ?? null });

/** Pastes the box's text of one session; if Rust refuses, closes that same session with that same text (captured now, so a late refusal never closes a newer preview). Input: the session the page shows and the box's text. */
export const applyPreview = (session: number, text: string) => invoke("preview_apply", { text, session }).catch(() => closePreview(session, text));

/** A spellchecker over a very long text costs more than it helps. */
const SPELLCHECK_MAX = 20_000;

/** The word count as shown under a block. Input: the text. Return: "N words" ("1 word"). */
export const wordCount = (text: string) => {
  const n = text.trim().split(/\s+/).filter(Boolean).length;
  return `${n} ${n === 1 ? "word" : "words"}`;
};

/** A label over a block, with a one-line note under it (the reference form's helper text). The block's size is the column's, never the text's. The frame clips while the scroller inside it carries the padding, so text never runs under the border and the scrollbar sits inside the frame; the scroller takes focus (arrow keys scroll it) and is named by its label, since a scrolling region must be reachable by keyboard. Input: the label, the note, whether the text is muted, whether the block is bare (its child scrolls itself) and the content. */
export function Block(p: { label: string; note?: string; muted?: boolean; bare?: boolean; children?: JSX.Element }) {
  const heading = createUniqueId();
  return (
    <section class="flex min-h-0 min-w-0 flex-1 flex-col gap-2">
      <h2 id={heading} class="t-label [font-weight:var(--w-strong)]">{p.label}</h2>
      <div class="t-label min-h-0 flex-1 overflow-hidden rounded-xl border border-line-strong bg-field select-text" classList={{ "!text-ink/75": p.muted }}>
        <div class="h-full" classList={{ "thin-scroll fade-edges overflow-auto px-3 py-2.5": !p.bare }} tabindex={p.bare ? undefined : 0} role={p.bare ? undefined : "region"} aria-labelledby={heading}>{p.children}</div>
      </div>
      <p class="t-hint min-h-4 px-0.5">{p.note}</p>
    </section>
  );
}

/** Read-only text, "Empty" when there is none. Input: the text. */
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
      class="thin-scroll fade-edges fade-in block h-full w-full resize-none overflow-auto bg-transparent px-3 py-2.5 leading-5 outline-none"
    />
  );
}

/** Five bars in a wave while something loads. The box has a fixed size and the bars move only by transform and opacity, so the wave never changes the layout around it; `label` is read by screen readers and not shown. */
export const Working = (p: { label: string }) => (
  <div class="flex h-full items-center justify-center text-muted" role="status" aria-label={p.label}>
    <span class="flex h-4 w-[27px] items-center justify-between" aria-hidden="true">
      <For each={[0, 1, 2, 3, 4]}>{(i) => <span class="wave-bar" style={{ "animation-delay": `${i * 120}ms` }} />}</For>
    </span>
  </div>
);

/** The shared footer with "Text" at the left and one action at the right: Paste (or `verb`) with Ctrl ↵ when there is something to paste, otherwise Cancel with Esc. Input: what Paste and Cancel do, and Paste's label. Return: the footer. */
export function Footer(p: { onPaste?: () => void; onCancel: () => void; verb?: string }) {
  return <ActionFooter title="Text" primary={p.onPaste ? { label: p.verb ?? "Paste", keys: ["Ctrl", "↵"], onClick: () => p.onPaste?.() } : { label: "Cancel", keys: ["Esc"], onClick: p.onCancel }} />;
}
