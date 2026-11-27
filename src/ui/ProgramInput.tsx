/**
 * File: ui/ProgramInput.tsx
 * Purpose: A list of programs as chips, filled from a searchable dropdown only: the dropdown lists every program on the PC by the name people know (the Start menu's "All apps", plus any running program it lacks), typing narrows it,
 * and choosing one adds it. Nothing is typed in by hand, so a list can only hold programs that exist.
 * Contents: useApps — reads the app list through `read`, only the latest read applies, a malformed reply leaves a sentence and no apps; ProgramInput — header, chips and the dropdown, which reads the list again each time it opens.
 */
import * as Combobox from "@kobalte/core/combobox";
import { ChevronDown } from "lucide-solid";
import { createMemo, createSignal, For, onMount, Show } from "solid-js";
import { read, reason, strings } from "../ipc";
import type { SettingId } from "../search";
import { KEY_RING } from "./controls";
import { Chip } from "./TagInput";

/** A saved entry is `Name<TAB>exe` (from the picker); only the name is shown, the executable tells programs apart. */
const label = (entry: string) => entry.split("\t")[0] ?? entry;
/** The executable of a saved entry, if it has one. Input: the entry. Return: the executable or undefined. */
const exeOf = (entry: string) => entry.split("\t")[1];

/** What the user sees when the app list reply has the wrong shape; opening the dropdown reads it again. */
const UNREADABLE = "Cue could not read the list of programs. Open the dropdown to try again.";

/** The app list and its error sentence. Return: the apps, the sentence ("" when fine) and `load`, which reads the list again; only the latest read applies its reply. */
function useApps() {
  const [apps, setApps] = createSignal<string[]>([]);
  const [error, setError] = createSignal("");
  let latest = 0;
  const load = () => {
    const n = ++latest;
    const apply = (list: string[], sentence: string) => { if (n === latest) { setApps(list); setError(sentence); } };
    void read("list_apps", strings, UNREADABLE).then((list) => apply(list, ""), (e) => apply([], reason(e)));
  };
  return { apps, error, load };
}

type Props = { value: string[]; onChange: (value: string[]) => void; title: string; hint?: string; id?: SettingId };

/** The program list setting. Input: the saved entries, the change handler, the title, an optional hint and setting id. Return: the field. */
export function ProgramInput(p: Props) {
  const { apps, error, load } = useApps();
  onMount(load);
  const [open, setOpen] = createSignal(false);
  // Read again each time the dropdown opens, so a program installed since shows up and a failed read can be retried.
  const show = (now: boolean) => { setOpen(now); if (now) load(); };
  // The same program is taken by its executable when it has one (two Start menu names can run the same file), else by its name.
  const taken = (entry: string) => p.value.some((v) => (exeOf(entry) && exeOf(v) === exeOf(entry)) || label(v).toLowerCase() === label(entry).toLowerCase());
  const choices = createMemo(() => apps().filter((name) => !taken(name)));
  return (
    <div class="flex flex-col gap-2.5 px-3 py-[13px]" data-setting={p.id}>
      <div class="flex flex-col gap-0.5">
        <div class="t-label">{p.title}</div>
        <Show when={p.hint}><div class="t-hint max-w-[52ch]">{p.hint}</div></Show>
      </div>
      <Show when={p.value.length}>
        <div class="flex flex-wrap gap-1.5">
          <For each={p.value}>{(name) => <Chip name={name} label={label(name)} onRemove={() => p.onChange(p.value.filter((n) => n !== name))} />}</For>
        </div>
      </Show>
      <Combobox.Root<string>
        options={choices()}
        optionTextValue={label}
        optionLabel={label}
        value={null}
        onChange={(name) => name && p.onChange([...p.value, name])}
        placeholder="Add a program"
        open={open()}
        // Opened by pressing the field, not by focus: closing returns the focus to the field, which would open it again.
        onOpenChange={show}
        // Kobalte opens by keyboard or typing only through onOpenChange, which it skips while the list is empty; allowing that keeps a failed read retryable.
        allowsEmptyCollection
        placement="bottom-start"
        gutter={4}
        itemComponent={(item) => (
          <Combobox.Item item={item.item} class="t-control flex h-7 shrink-0 min-w-0 cursor-default items-center rounded-md px-2.5 outline-none data-[highlighted]:bg-hover data-[selected]:bg-selected">
            <Combobox.ItemLabel class="truncate">{label(item.item.rawValue)}</Combobox.ItemLabel>
          </Combobox.Item>
        )}
      >
        <Combobox.Control aria-label="Add a program" onPointerDown={() => open() || show(true)} class={`t-control flex h-8 w-72 max-w-full items-center gap-1.5 rounded-lg border border-line bg-control pl-2.5 pr-2 ${KEY_RING}`}>
          <Combobox.Input class="h-full min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted" />
          <Combobox.Trigger class="shrink-0"><Combobox.Icon><ChevronDown size={14} class="text-muted" /></Combobox.Icon></Combobox.Trigger>
        </Combobox.Control>
        <Combobox.Portal>
          <Combobox.Content class="popup z-50 w-72 max-w-[calc(100vw-2rem)] rounded-lg border border-line bg-popup p-1 shadow-popup backdrop-blur-xl">
            <Combobox.Listbox class="thin-scroll flex max-h-[min(11rem,var(--kb-popper-content-available-height,11rem))] flex-col gap-px overflow-y-auto outline-none" />
          </Combobox.Content>
        </Combobox.Portal>
      </Combobox.Root>
      <Show when={error()}><p class="t-hint !text-danger">{error()}</p></Show>
    </div>
  );
}
