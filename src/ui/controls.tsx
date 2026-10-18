/**
 * Purpose: The interactive controls of the kit.
 * Contents: Toggle (Kobalte Switch, slate when on), ValueField (a label over a read-only value), Select (Kobalte; value plus chevron, or a bordered form field), Segmented and Tabs
 * (Kobalte ToggleGroup), TextField, TextArea, KEY_RING — keyboard-only focus ring for controls whose real input is visually hidden.
 */
import * as K from "@kobalte/core/select";
import * as Switch from "@kobalte/core/switch";
import * as ToggleGroup from "@kobalte/core/toggle-group";
import { ChevronDown } from "lucide-solid";
import { For } from "solid-js";

/** Focus ring for controls with a hidden input; shows only after keyboard use (see src/focus.ts). */
export const KEY_RING = "[:root[data-modality=keyboard]_&:has(:focus-visible)]:outline-2 [:root[data-modality=keyboard]_&:has(:focus-visible)]:outline-offset-2 [:root[data-modality=keyboard]_&:has(:focus-visible)]:outline-accent";

export const Toggle = (p: { checked: boolean; onChange: (v: boolean) => void; label: string }) => (
  <Switch.Root checked={p.checked} onChange={p.onChange} aria-label={p.label} class={`group rounded-full ${KEY_RING}`}>
    <Switch.Input />
    <Switch.Control class="switch-track flex h-4 w-7 items-center rounded-full bg-selected p-[2.5px] data-[checked]:bg-switch-on">
      <Switch.Thumb class="switch-thumb size-[11px] rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0/0.3),0_0_0_0.5px_rgb(0_0_0/0.06)] data-[checked]:translate-x-3" />
    </Switch.Control>
  </Switch.Root>
);

/** `field` draws it as a bordered form field that fills its container (for forms); otherwise it is the plain value with a chevron of a settings row. */
export function Select(p: { value: string; options: string[]; onChange: (v: string) => void; label: string; field?: boolean }) {
  return (
    <K.Root
      options={p.options}
      value={p.value}
      onChange={(v) => v && p.onChange(v)}
      disallowEmptySelection
      aria-label={p.label}
      placement={p.field ? "bottom-start" : "bottom-end"}
      sameWidth={p.field}
      gutter={p.field ? 4 : 6}
      itemComponent={(ip) => (
        <K.Item item={ip.item} class="t-control flex h-7 min-w-0 cursor-default items-center rounded-md px-2.5 outline-none data-[highlighted]:bg-hover data-[selected]:bg-selected">
          <K.ItemLabel class="truncate">{ip.item.rawValue}</K.ItemLabel>
        </K.Item>
      )}
    >
      <K.Trigger
        class="t-control flex h-7 items-center gap-1.5 rounded-md data-[expanded]:bg-control"
        classList={{ "w-full justify-between border border-line bg-control px-2.5": p.field, "max-w-[16rem] justify-end pl-2 pr-1": !p.field }}
      >
        <K.Value<string> class="truncate">{(s) => s.selectedOption()}</K.Value>
        <K.Icon class="shrink-0"><ChevronDown size={14} class="text-muted" /></K.Icon>
      </K.Trigger>
      <K.Portal>
        <K.Content class="popup z-50 min-w-40 max-w-[20rem] rounded-lg border border-line bg-popup p-1 shadow-popup backdrop-blur-xl">
          <K.Listbox class="flex max-h-[min(14rem,var(--kb-popper-content-available-height,14rem))] flex-col gap-px overflow-y-auto outline-none" />
        </K.Content>
      </K.Portal>
    </K.Root>
  );
}

export function Segmented(p: { value: string; options: string[]; onChange: (v: string) => void; label: string }) {
  return (
    <ToggleGroup.Root value={p.value} onChange={(v) => v && p.onChange(v)} aria-label={p.label} class="flex gap-px rounded-md bg-control p-0.5">
      <For each={p.options}>
        {(o) => (
          <ToggleGroup.Item value={o} class="t-control h-6 rounded px-3 text-muted data-[pressed]:bg-selected data-[pressed]:text-ink">
            {o}
          </ToggleGroup.Item>
        )}
      </For>
    </ToggleGroup.Root>
  );
}

/** Text tabs without a track, as for Overview and Activity: the selected one has a filled rounded rectangle. */
export function Tabs(p: { value: string; options: string[]; onChange: (v: string) => void; label: string }) {
  return (
    <ToggleGroup.Root value={p.value} onChange={(v) => v && p.onChange(v)} aria-label={p.label} class="flex gap-1">
      <For each={p.options}>
        {(o) => (
          <ToggleGroup.Item value={o} class="t-control h-6 rounded-md px-2.5 text-muted data-[pressed]:bg-selected data-[pressed]:text-ink">
            {o}
          </ToggleGroup.Item>
        )}
      </For>
    </ToggleGroup.Root>
  );
}

/** A label over a read-only, single-line value in a bordered field (the reference form's field look); a long value truncates, the full text in `title`. */
export function ValueField(p: { label: string; value: string }) {
  return (
    <div class="flex min-w-0 flex-col gap-2 pb-3">
      <span class="t-label [font-weight:var(--w-strong)]">{p.label}</span>
      <div class="t-label flex h-[33px] min-w-0 items-center rounded-xl border border-line-strong bg-field px-3" title={p.value}>
        <span class="truncate">{p.value}</span>
      </div>
    </div>
  );
}

/** Single-line text input that shrinks with its row; the caret is the focus cue, the ring shows after keyboard use only. */
export function TextField(p: { value: string; onInput: (v: string) => void; label: string; placeholder?: string; autofocus?: boolean }) {
  return (
    <div class={`min-w-0 flex-1 rounded-md ${KEY_RING}`}>
      <input
        ref={(el) => { if (p.autofocus) queueMicrotask(() => el.focus()); }}
        aria-label={p.label}
        value={p.value}
        placeholder={p.placeholder}
        spellcheck={false}
        autocomplete="off"
        onInput={(e) => p.onInput(e.currentTarget.value)}
        class="t-control h-7 w-full min-w-0 rounded-md border border-line bg-control px-2.5 outline-none placeholder:text-muted"
      />
    </div>
  );
}

/** Multi-line text input for longer text: a fixed size set by the app, scrolling inside, never resizable by the user. */
export function TextArea(p: { value: string; onInput: (v: string) => void; label: string; placeholder?: string; tall?: boolean }) {
  return (
    <div class={`min-w-0 flex-1 rounded-md ${KEY_RING}`}>
      <textarea
        aria-label={p.label}
        value={p.value}
        placeholder={p.placeholder}
        spellcheck={false}
        onInput={(e) => p.onInput(e.currentTarget.value)}
        class="t-control w-full min-w-0 resize-none overflow-y-auto rounded-md border border-line bg-control px-2.5 py-1.5 outline-none placeholder:text-muted"
        classList={{ "h-28": !p.tall, "h-56": p.tall }}
      />
    </div>
  );
}
