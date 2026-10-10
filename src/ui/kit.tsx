/**
 * Purpose: Shared settings components so every page is assembled, not hand-styled.
 * Contents: Page, Section, Card, Row, Button, Toggle (Kobalte Switch), Select (Kobalte), Segmented (Kobalte ToggleGroup), Progress, Badge, Meter (1 to 5 segments), IconButton, TextField,
 * KEY_RING — keyboard-only focus ring for controls whose real input is visually hidden.
 */
import * as K from "@kobalte/core/select";
import * as Switch from "@kobalte/core/switch";
import * as ToggleGroup from "@kobalte/core/toggle-group";
import { ChevronDown } from "lucide-solid";
import { For, type JSX } from "solid-js";

/** Focus ring for controls with a hidden input; shows only after keyboard use (see src/focus.ts). */
export const KEY_RING = "[:root[data-modality=keyboard]_&:has(:focus-visible)]:outline-2 [:root[data-modality=keyboard]_&:has(:focus-visible)]:outline-offset-2 [:root[data-modality=keyboard]_&:has(:focus-visible)]:outline-accent";

export const Page = (p: { children: JSX.Element }) => (
  <div class="flex flex-col gap-6 p-4">{p.children}</div>
);

export const Section = (p: { title?: string; children: JSX.Element }) => (
  <section class="flex flex-col gap-2">
    {p.title && <h2 class="t-label px-3 pb-0.5">{p.title}</h2>}
    {p.children}
  </section>
);

export const Card = (p: { children: JSX.Element }) => (
  <div class="divide-y divide-line overflow-hidden rounded-[10px] bg-card">{p.children}</div>
);

export const Row = (p: { label: string; hint?: string; children: JSX.Element }) => (
  <div class="flex items-center justify-between gap-6 px-3 py-[13px]">
    <div class="flex min-w-0 flex-col gap-0.5">
      <div class="t-label">{p.label}</div>
      {p.hint && <div class="t-hint max-w-[52ch]">{p.hint}</div>}
    </div>
    <div class="shrink-0">{p.children}</div>
  </div>
);

export const Toggle = (p: { checked: boolean; onChange: (v: boolean) => void; label: string }) => (
  <Switch.Root checked={p.checked} onChange={p.onChange} aria-label={p.label} class={`group rounded-full ${KEY_RING}`}>
    <Switch.Input />
    <Switch.Control class="switch-track flex h-4 w-7 items-center rounded-full bg-selected p-[2.5px] data-[checked]:bg-accent">
      <Switch.Thumb class="switch-thumb size-[11px] rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0/0.3),0_0_0_0.5px_rgb(0_0_0/0.06)] data-[checked]:translate-x-3" />
    </Switch.Control>
  </Switch.Root>
);

export function Select(p: { value: string; options: string[]; onChange: (v: string) => void; label: string }) {
  return (
    <K.Root
      options={p.options}
      value={p.value}
      onChange={(v) => v && p.onChange(v)}
      disallowEmptySelection
      aria-label={p.label}
      placement="bottom-end"
      gutter={6}
      itemComponent={(ip) => (
        <K.Item item={ip.item} class="t-control flex h-7 min-w-0 cursor-default items-center rounded-md px-2.5 outline-none data-[highlighted]:bg-hover data-[selected]:bg-selected">
          <K.ItemLabel class="truncate" title={ip.item.rawValue}>{ip.item.rawValue}</K.ItemLabel>
        </K.Item>
      )}
    >
      <K.Trigger class="t-control flex h-7 max-w-[16rem] items-center justify-end gap-1.5 rounded-md pl-2 pr-1" title={p.value}>
        <K.Value<string> class="truncate">{(s) => s.selectedOption()}</K.Value>
        <K.Icon class="shrink-0"><ChevronDown size={14} class="text-muted" /></K.Icon>
      </K.Trigger>
      <K.Portal>
        <K.Content class="popup z-50 min-w-40 max-w-[20rem] rounded-lg border border-line bg-card/80 p-1 shadow-2xl backdrop-blur-xl">
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

/** Quiet text button; `danger` colours the label only (the one place danger appears outside errors). */
export const Button = (p: { onClick: () => void; danger?: boolean; children: JSX.Element }) => (
  <button onClick={() => p.onClick()} class="t-control h-7 rounded-md bg-control px-3" classList={{ "!text-danger": p.danger }}>
    {p.children}
  </button>
);


/** Thin determinate bar for downloads; `value` is 0..100. */
export const Progress = (p: { value: number }) => (
  <div class="h-1 w-24 overflow-hidden rounded-full bg-selected" role="progressbar" aria-valuenow={p.value} aria-valuemin={0} aria-valuemax={100}>
    <div class="h-full rounded-full bg-accent" style={{ width: `${p.value}%` }} />
  </div>
);

/** Square 28 px icon button for quiet actions (add, remove, reset). `label` is its accessible name and tooltip. */
export const IconButton = (p: { label: string; onClick: () => void; children: JSX.Element }) => (
  <button onClick={() => p.onClick()} aria-label={p.label} title={p.label} class="grid size-7 shrink-0 place-items-center rounded-md bg-control text-muted">
    {p.children}
  </button>
);

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
        class="t-control h-7 w-full min-w-0 rounded-md bg-control px-2.5 outline-none placeholder:text-muted"
      />
    </div>
  );
}

/** Small pill label; `accent` marks the one that is active. */
export const Badge = (p: { accent?: boolean; children: JSX.Element }) => (
  <span class="t-meta shrink-0 rounded-full bg-selected px-2 leading-5" classList={{ "!text-accent": p.accent }}>{p.children}</span>
);

/** A labelled 1 to 5 rating drawn as five short bars. */
export const Meter = (p: { label: string; value: number }) => (
  <div class="flex items-center justify-end gap-2" role="img" aria-label={`${p.label} ${p.value} of 5`}>
    <span class="t-micro">{p.label}</span>
    <div class="flex gap-0.5">
      <For each={[1, 2, 3, 4, 5]}>{(n) => <span class="h-1 w-3 rounded-full" classList={{ "bg-accent": n <= p.value, "bg-selected": n > p.value }} />}</For>
    </div>
  </div>
);
