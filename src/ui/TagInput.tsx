/**
 * Purpose: A list of short names as chips in one bordered field (program names, for example): type a name and press Enter or a comma to add it, a chip's x removes it, Backspace in the empty field removes the last chip.
 * Contents: TagInput — header text, the chips and the input; the caller decides what a name looks like through `clean`.
 */
import { X } from "lucide-solid";
import { createSignal, For, Show } from "solid-js";
import type { SettingId } from "../search";
import { KEY_RING } from "./controls";

function Chip(p: { name: string; onRemove: () => void }) {
  return (
    <span class="t-control flex h-6 max-w-full items-center gap-1 rounded-md bg-selected pl-2 pr-1">
      <span class="truncate">{p.name}</span>
      <button aria-label={`Remove ${p.name}`} onClick={() => p.onRemove()} class="grid size-4 place-items-center rounded text-muted">
        <X size={12} stroke-width={2} />
      </button>
    </span>
  );
}

export function TagInput(p: {
  value: string[];
  onChange: (value: string[]) => void;
  title?: string;
  hint?: string;
  placeholder: string;
  label: string;
  clean?: (text: string) => string;
  id?: SettingId;
}) {
  const [draft, setDraft] = createSignal("");
  const add = () => {
    const name = (p.clean ?? ((t: string) => t.trim()))(draft());
    setDraft("");
    if (name && !p.value.includes(name)) p.onChange([...p.value, name]);
  };
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      add();
    } else if (e.key === "Backspace" && !draft() && p.value.length) {
      p.onChange(p.value.slice(0, -1));
    }
  };
  return (
    <div class="flex flex-col gap-2 px-3 py-[13px]" data-setting={p.id}>
      <Show when={p.title}>
        <div class="flex flex-col gap-0.5">
          <div class="t-label">{p.title}</div>
          <Show when={p.hint}><div class="t-hint max-w-[52ch]">{p.hint}</div></Show>
        </div>
      </Show>
      <div class={`flex min-h-10 flex-wrap items-center gap-1.5 rounded-lg border border-line bg-control p-1.5 ${KEY_RING}`}>
        <For each={p.value}>{(name) => <Chip name={name} onRemove={() => p.onChange(p.value.filter((n) => n !== name))} />}</For>
        <input
          aria-label={p.label}
          value={draft()}
          placeholder={p.value.length ? "" : p.placeholder}
          spellcheck={false}
          autocomplete="off"
          onInput={(e) => setDraft(e.currentTarget.value)}
          onKeyDown={keys}
          onBlur={add}
          class="t-control h-6 min-w-[8ch] flex-1 bg-transparent px-1 outline-none placeholder:text-muted"
        />
      </div>
    </div>
  );
}
