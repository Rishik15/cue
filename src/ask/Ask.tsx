/**
 * File: ask/Ask.tsx
 * Purpose: The snippet question window's page: one field per `{argument:Label}` the snippet being typed asks for (a text box, or a list when the snippet gave choices). Enter or Insert answers once the questions are loaded, Escape cancels; the shared footer names the window and shows Cancel and Insert.
 * Contents: Argument — one question; isArgument / isArguments — the check of Rust's list; answer — send the answers; Ask — the window; Field — one question.
 */
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { ActionFooter, Select, TextField } from "../ui/kit";
import { composing } from "../ui/keys";

/** One question: its label and, when the snippet gave them, the choices. */
type Argument = { label: string; choices: string[] };

/** Whether a value is one question with the right types. Input: the value. Return: the answer. */
const isArgument = (a: unknown): a is Argument => {
  if (typeof a !== "object" || a === null) return false;
  const { label, choices } = a as Record<string, unknown>;
  return typeof label === "string" && Array.isArray(choices) && choices.every((c) => typeof c === "string");
};

/** Whether a value is a list of questions. Input: the value. Return: the answer. */
const isArguments = (v: unknown): v is Argument[] => Array.isArray(v) && v.every(isArgument);

/** Sends the answers to Rust (null cancels); if that fails the window closes itself, so it is never stuck open. Input: the answers or null. Return: a promise that never rejects. */
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

/** The window: shown first, then the questions are fetched (Rust gives the window the keyboard on that call); an empty or malformed list closes it. */
export function Ask() {
  const [args, setArgs] = createSignal<Argument[]>([]);
  const [values, setValues] = createSignal<string[]>([]);
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
        if (!isArguments(list) || !list.length) return void getCurrentWindow().close();
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
      <ActionFooter title="Snippet" secondary={[{ label: "Cancel", keys: ["Esc"], onClick: cancel }]} primary={{ label: "Insert", keys: ["↵"], onClick: submit }} />
    </div>
  );
}
