/**
 * Purpose: Shared settings components so every page is assembled, not hand-styled.
 * Contents: Page, Section, Card, Row, Toggle (Kobalte Switch), Select (Kobalte), Segmented (Kobalte ToggleGroup),
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
    <Switch.Control class="switch-track flex h-4 w-7 items-center rounded-full bg-selected p-0.5 data-[checked]:bg-accent">
      <Switch.Thumb class="switch-thumb h-3 w-3 rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0/0.35)] group-active:w-3.5 data-[checked]:translate-x-3 data-[checked]:group-active:translate-x-2.5" />
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
        <K.Item item={ip.item} class="t-control flex h-7 cursor-default items-center rounded-md px-2.5 outline-none data-[highlighted]:bg-hover data-[selected]:bg-selected">
          <K.ItemLabel>{ip.item.rawValue}</K.ItemLabel>
        </K.Item>
      )}
    >
      <K.Trigger class="t-control flex h-7 items-center justify-end gap-1.5 rounded-md pl-2 pr-1">
        <K.Value<string>>{(s) => s.selectedOption()}</K.Value>
        <K.Icon><ChevronDown size={14} class="text-muted" /></K.Icon>
      </K.Trigger>
      <K.Portal>
        <K.Content class="popup z-50 min-w-40 rounded-lg border border-line bg-card/80 p-1 shadow-2xl backdrop-blur-xl">
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
