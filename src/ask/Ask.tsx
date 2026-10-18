/**
 * Purpose: The snippet question window's page: one field per `{argument:Label}` the snippet being typed asks for (a text box, or a list when the snippet gave choices). Enter inserts, Escape cancels.
 * Contents: Ask — the window; Field — one question.
 */
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { ActionHint, Select, TextField } from "../ui/kit";

type Argument = { label: string; choices: string[] };

const isArgument = (a: unknown): a is Argument => {
  if (typeof a !== "object" || a === null) return false;
  const { label, choices } = a as Record<string, unknown>;
  return typeof label === "string" && Array.isArray(choices) && choices.every((c) => typeof c === "string");
};

const isArguments = (v: unknown): v is Argument[] => Array.isArray(v) && v.every(isArgument);

const answer = (values: string[] | null) => invoke("ask_submit", { values }).catch(() => getCurrentWindow().close().catch(() => undefined));

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

export function Ask() {
  const [args, setArgs] = createSignal<Argument[]>([]);
  const [values, setValues] = createSignal<string[]>([]);
  const submit = () => void answer(values());
  const cancel = () => void answer(null);
  const keys = (e: KeyboardEvent) => {
    if (e.isComposing) return; // Enter / Escape belong to the input method
    if (e.key === "Escape") cancel();
    // Not before the questions are loaded (empty answers would be inserted), and not on the list, where Enter picks a choice.
    const onList = (e.target as HTMLElement).closest("button, [role=listbox], [role=option]");
    if (e.key === "Enter" && args().length && !onList) submit();
  };
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
    // Shown first, then the questions are fetched: Rust gives the window the keyboard on that call.
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
      <footer class="flex h-10 shrink-0 items-center justify-end gap-4 border-t border-line pl-[13px] pr-[11px]">
        <ActionHint label="Cancel" keys={["Esc"]} onClick={cancel} />
        <ActionHint label="Insert" keys={["↵"]} onClick={submit} />
      </footer>
    </div>
  );
}
