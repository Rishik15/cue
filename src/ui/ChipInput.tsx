/**
 * Purpose: The custom vocabulary as chips in one bordered field, like Raycast's Vocabulary: type a word and press Enter to add it, or
 * `from -> to` to replace a phrase; a chip's x removes it; Backspace in the empty field removes the last chip.
 * Contents: ChipInput — header text, the chips and the input.
 */
import { X } from "lucide-solid";
import { createSignal, For } from "solid-js";
import type { SettingId } from "../search";
import type { Entry } from "../vocabulary";
import { KEY_RING } from "./controls";

/** The arrow is the file format's separator, so it cannot appear inside a word. */
const plain = (text: string) => text.replace(/->/g, " ").replace(/\s+/g, " ").trim();

function parse(text: string): Entry | null {
  const [word = "", ...rest] = text.split("->");
  const entry = { word: plain(word), replacement: plain(rest.join(" ")) };
  return entry.word ? entry : null;
}

const label = (e: Entry) => (e.replacement ? `${e.word} → ${e.replacement}` : e.word);

export function ChipInput(p: { entries: Entry[]; onChange: (entries: Entry[]) => void; title: string; hint: string; id?: SettingId }) {
  const [draft, setDraft] = createSignal("");
  const add = () => {
    const entry = parse(draft());
    setDraft("");
    if (!entry) return;
    // A word typed again replaces its old entry, so there is one chip per word.
    p.onChange([...p.entries.filter((e) => e.word.toLowerCase() !== entry.word.toLowerCase()), entry]);
  };
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      add();
    } else if (e.key === "Backspace" && !draft() && p.entries.length) {
      p.onChange(p.entries.slice(0, -1));
    }
  };
  return (
    <div class="flex flex-col gap-2 px-3 py-[13px]" data-setting={p.id}>
      <div class="flex flex-col gap-0.5">
        <div class="t-label">{p.title}</div>
        <div class="t-hint max-w-[52ch]">{p.hint}</div>
      </div>
      <div class={`flex min-h-10 flex-wrap items-center gap-1.5 rounded-lg border border-line bg-control p-1.5 ${KEY_RING}`}>
        <For each={p.entries}>
          {(entry) => (
            <span class="t-control flex h-6 max-w-full items-center gap-1 rounded-md bg-selected pl-2 pr-1">
              <span class="truncate">{label(entry)}</span>
              <button aria-label={`Remove ${entry.word}`} onClick={() => p.onChange(p.entries.filter((e) => e !== entry))} class="grid size-4 place-items-center rounded text-muted">
                <X size={12} stroke-width={2} />
              </button>
            </span>
          )}
        </For>
        <input
          aria-label="Add a word"
          value={draft()}
          placeholder={p.entries.length ? "" : "Add a word"}
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
