/**
 * Purpose: Theme gallery: one card per theme family showing its dark and light variants side by side
 * (the settings-row pattern Raycast uses for Window Mode). The Appearance mode picks which variant is live.
 * Contents: Miniature — mini settings window in a variant's colors; ThemePicker — radio group of family cards;
 * selected card = accent ring + full-ink name (no tick), others hairline + muted name.
 */
import * as RadioGroup from "@kobalte/core/radio-group";
import { For } from "solid-js";
import { FAMILIES, type Roles } from "../themes";
import { KEY_RING } from "./kit";

/** Mini Cue settings window in a variant's colors: header band, sidebar with a selected item, two cards with rows and toggles. */
const Miniature = (p: { c: Roles }) => (
  <svg viewBox="0 0 28 20" aria-hidden="true" class="block h-auto w-full rounded-lg">
    <rect width="28" height="20" fill={p.c.bg} />
    <rect y="2.4" width="9" height="17.6" fill={p.c.sidebar} />
    <rect y="2.4" width="28" height="0.2" fill={p.c.ink} opacity="0.12" />
    <rect x="9" y="2.4" width="0.2" height="17.6" fill={p.c.ink} opacity="0.12" />
    <rect x="1.4" y="1" width="3" height="0.7" rx="0.35" fill={p.c.ink} opacity="0.35" />
    <rect x="21.6" y="0.9" width="1.2" height="0.9" rx="0.3" fill={p.c.ink} opacity="0.3" />
    <rect x="23.6" y="0.9" width="1.2" height="0.9" rx="0.3" fill={p.c.ink} opacity="0.3" />
    <rect x="25.6" y="0.9" width="1.2" height="0.9" rx="0.3" fill={p.c.ink} opacity="0.3" />
    <rect x="1" y="4.2" width="7" height="2.4" rx="0.9" fill={p.c.ink} opacity="0.14" />
    <circle cx="2.4" cy="5.4" r="0.55" fill={p.c.ink} opacity="0.7" />
    <rect x="3.7" y="5.1" width="3" height="0.6" rx="0.3" fill={p.c.ink} opacity="0.7" />
    <circle cx="2.4" cy="8.6" r="0.55" fill={p.c.ink} opacity="0.4" />
    <rect x="3.7" y="8.3" width="3.2" height="0.6" rx="0.3" fill={p.c.ink} opacity="0.4" />
    <circle cx="2.4" cy="11" r="0.55" fill={p.c.ink} opacity="0.4" />
    <rect x="3.7" y="10.7" width="2.6" height="0.6" rx="0.3" fill={p.c.ink} opacity="0.4" />
    <rect x="10.8" y="4.4" width="16" height="7.2" rx="1.3" fill={p.c.card} />
    <rect x="10.8" y="7.9" width="16" height="0.15" fill={p.c.ink} opacity="0.1" />
    <rect x="12.2" y="5.8" width="5.4" height="0.8" rx="0.4" fill={p.c.ink} opacity="0.75" />
    <rect x="22.6" y="5.4" width="2.9" height="1.7" rx="0.85" fill={p.c.accent} />
    <rect x="12.2" y="9.3" width="6.4" height="0.8" rx="0.4" fill={p.c.ink} opacity="0.75" />
    <rect x="22.6" y="8.9" width="2.9" height="1.7" rx="0.85" fill={p.c.control} />
    <rect x="10.8" y="13" width="16" height="5" rx="1.3" fill={p.c.card} />
    <rect x="12.2" y="14.5" width="4.6" height="0.8" rx="0.4" fill={p.c.ink} opacity="0.75" />
    <rect x="12.2" y="16" width="7.4" height="0.55" rx="0.27" fill={p.c.muted} />
    <rect x="22" y="14.4" width="3.5" height="2.2" rx="0.7" fill={p.c.control} />
  </svg>
);

export function ThemePicker(p: { label: string; hint?: string; value: string; onChange: (name: string) => void }) {
  return (
    <div class="flex flex-col gap-3 px-3 py-[13px]">
      <div class="flex flex-col gap-0.5">
        <div class="t-label">{p.label}</div>
        {p.hint && <div class="t-hint">{p.hint}</div>}
      </div>
      <RadioGroup.Root value={p.value} onChange={p.onChange} aria-label={p.label} class="grid grid-cols-3 gap-x-3 gap-y-5">
        <For each={FAMILIES}>
          {(f) => (
            <RadioGroup.Item value={f.name} class={`group flex flex-col gap-2 rounded-xl ${KEY_RING}`}>
              <RadioGroup.ItemInput />
              <RadioGroup.ItemControl class="grid grid-cols-2 gap-1.5 rounded-xl p-1.5 ring-1 ring-line data-[checked]:ring-2 data-[checked]:ring-accent">
                <Miniature c={f.dark} />
                <Miniature c={f.light} />
              </RadioGroup.ItemControl>
              <RadioGroup.ItemLabel class="t-control text-center text-muted group-data-[checked]:text-ink">{f.name}</RadioGroup.ItemLabel>
            </RadioGroup.Item>
          )}
        </For>
      </RadioGroup.Root>
    </div>
  );
}
