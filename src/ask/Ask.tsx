/**
 * File: ask/Ask.tsx
 * Purpose: The snippet question window's page: one field per `{argument:Label}` the snippet being typed asks for (a text box, or a list when the snippet gave choices). Enter or Insert answers once the questions are loaded, Escape cancels; the shared footer names the window and shows Cancel and Insert. No questions (the snippet is gone) closes the window; a reply Cue cannot read keeps it open with a sentence in the footer, so Cancel still works.
 * Contents: Argument — one question; isArguments — the check of Rust's list; MALFORMED — the sentence for a list Cue cannot read; answer — send the answers; Ask — the window; Field — one question.
 */
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { isStr, listOf, shaped, strings } from "../ipc";
import { ActionFooter, Select, TextField } from "../ui/kit";
import { composing } from "../ui/keys";

/** One question: its label and, when the snippet gave them, the choices. */
type Argument = { label: string; choices: string[] };

/** Whether a value is a list of questions, each a label and its choices. */
const isArguments = listOf((a: unknown): a is Argument => shaped<Argument>(a, { label: isStr, choices: strings }));

/** What the footer says when the questions arrive in a shape Cue cannot read. */
const MALFORMED = "Cue could not read the questions. Cancel and type the snippet again.";

/** Sends the answers to Rust (null cancels); `ask_submit` never refuses, so a failure is the bridge itself and the window closes, which Rust takes as a cancel, so it is never stuck open. Input: the answers or null. Return: a promise that never rejects. */
const answer = (values: string[] | null) => invoke("ask_submit", { values }).catch(() => getCurrentWindow().close().catch(() => undefined));

/** One question: its label over a text box, or over a list when it has choices; the first text box takes the keyboard. Input: the question, its value, whether it is the first and what typing does. */
function Field(p: { arg: Argument; value: string; first: boolean; onInput: (v: string) => void }) {
  return (
    <div class="flex flex-col gap-2">
      <h2 class="t-label [font-weight:var(--w-strong)] truncate">{p.arg.label}</h2>
      <Show when={p.arg.choices.length} fallback={<TextField label={p.arg.label} value={p.value} onInput={p.onInput} autofocus={p.first} />}>
        <Select field label={p.arg.label} value={p.value} options={p.arg.choices} onChange={p.onInput} />
      </Show>
    </div>
  );
}

/** The window: shown first, then the questions are fetched (Rust gives the window the keyboard on that call); no questions close it, a malformed list shows MALFORMED in the footer. */
export function Ask() {
  const [args, setArgs] = createSignal<Argument[]>([]);
  const [values, setValues] = createSignal<string[]>([]);
  const [note, setNote] = createSignal("");
  /** Inserts the answers; does nothing until the questions are loaded (Rust takes empty answers as a cancel). */
  const submit = () => { if (args().length) void answer(values()); };
  /** Cancels the snippet. */
  const cancel = () => void answer(null);
  /** Escape cancels; Enter inserts, not on the list, where Enter picks a choice; keys of an input method are its own. Input: the key event. */
  const keys = (e: KeyboardEvent) => {
    if (composing(e)) return;
    if (e.key === "Escape") cancel();
    const onList = (e.target as HTMLElement).closest("button, [role=listbox], [role=option]");
    if (e.key === "Enter" && !onList) submit();
  };
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
    getCurrentWindow()
      .show()
      .then(() => invoke<unknown>("ask_get"))
      .then((list) => {
        if (list === null || (Array.isArray(list) && !list.length)) return void getCurrentWindow().close();
        if (!isArguments(list)) return void setNote(MALFORMED);
        setArgs(list);
        setValues(list.map((a) => a.choices[0] ?? ""));
      }, cancel);
  });
  return (
    <div class="flex h-full flex-col bg-card/45">
      <header data-tauri-drag-region class="h-6 shrink-0" />
      <main class="thin-scroll flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-10 pb-3">
        <For each={args()}>{(arg, i) => <Field arg={arg} first={i() === 0} value={values()[i()] ?? ""} onInput={(v) => setValues(values().map((old, at) => (at === i() ? v : old)))} />}</For>
      </main>
      <ActionFooter title={note() || "Snippet"} secondary={[{ label: "Cancel", keys: ["Esc"], onClick: cancel }]} primary={{ label: "Insert", keys: ["↵"], onClick: submit }} />
    </div>
  );
}
