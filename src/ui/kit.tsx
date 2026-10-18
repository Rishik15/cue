/**
 * Purpose: The public face of the UI kit: page layout and rows here, controls in `controls`, the footer in `footer`.
 * Contents: Page, Section, Card, Row, ListRow (truncating title and subtitle, optional click), Hint (a note or empty state inside a card), AddRow, Button, Progress, IconButton,
 * PlusButton and MinusButton (small list buttons), Badge, Meter (1 to 5 segments); re-exports of the controls and footer so pages import from one place.
 */
import { CirclePlus, Minus, Plus } from "lucide-solid";
import { For, type JSX } from "solid-js";
import type { SettingId } from "../search";

export * from "./controls";
export * from "./footer";

/** The page's padding leaves room under the last card, so nothing sits against the footer or the window edge. */
export const Page = (p: { children: JSX.Element }) => (
  <div class="flex flex-col gap-6 px-4 pb-6 pt-4">{p.children}</div>
);

export const Section = (p: { title?: string; id?: SettingId; children: JSX.Element }) => (
  <section class="flex flex-col gap-2" data-setting={p.id}>
    {p.title && <h2 class="t-control px-3 pb-0.5 text-muted">{p.title}</h2>}
    {p.children}
  </section>
);

export const Card = (p: { children: JSX.Element }) => (
  <div class="divide-y divide-line overflow-hidden rounded-[10px] bg-card">{p.children}</div>
);

/** `id` is the setting's id in the search index (`src/search.ts`), so search can scroll to the row. */
export const Row = (p: { label: string; hint?: string; id?: SettingId; children: JSX.Element }) => (
  <div class="flex items-center justify-between gap-6 px-3 py-[13px]" data-setting={p.id}>
    <div class="flex min-w-0 flex-col gap-0.5">
      <div class="t-label">{p.label}</div>
      {p.hint && <div class="t-hint max-w-[52ch]">{p.hint}</div>}
    </div>
    <div class="shrink-0">{p.children}</div>
  </div>
);

/** One entry of a list card: a title over a one-line subtitle (both truncate), an optional click target and a trailing control. */
export const ListRow = (p: { title: string; subtitle?: string; onClick?: () => void; children?: JSX.Element }) => (
  <div class="group/row flex items-center has-[button:enabled]:hover:bg-hover">
    <button disabled={!p.onClick} onClick={() => p.onClick?.()} class="flex min-w-0 flex-1 flex-col gap-0.5 py-[13px] pl-3 text-left disabled:cursor-default">
      <span class="t-label truncate">{p.title}</span>
      {p.subtitle && <span class="t-hint truncate">{p.subtitle}</span>}
    </button>
    {p.children && <div class="shrink-0 py-[13px] pl-6 pr-3">{p.children}</div>}
  </div>
);

/** A note row inside a card: an empty, loading or error state. `danger` colours it as an error. */
export const Hint = (p: { danger?: boolean; children: JSX.Element }) => (
  <p class="t-hint px-3 py-[13px]" classList={{ "!text-danger": p.danger }}>{p.children}</p>
);

/** The last row of a list card: adds an entry. The whole row is the click target. */
export const AddRow = (p: { label: string; onClick: () => void }) => (
  <button onClick={() => p.onClick()} class="t-label flex h-11 w-full items-center gap-3 px-3 text-left">
    <CirclePlus size={18} stroke-width={1.5} class="text-muted" />
    {p.label}
  </button>
);

/** Filled quiet button for one-off actions (Reset, Download); `danger` colours the label only. */
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

/** Square 28 px icon button for quiet actions (remove, reset). `label` is its accessible name. */
export const IconButton = (p: { label: string; onClick: () => void; children: JSX.Element }) => (
  <button onClick={() => p.onClick()} aria-label={p.label} class="grid size-7 shrink-0 place-items-center rounded-md bg-control text-muted">
    {p.children}
  </button>
);

/** Small square buttons that add and remove entries in a list card: the plus is 24 px with a 12 px glyph, the minus 18 px with a 10 px glyph, so they
 *  stay quiet next to the row text. */
export const PlusButton = (p: { label: string; onClick: () => void }) => (
  <button onClick={() => p.onClick()} aria-label={p.label} class="grid size-6 shrink-0 place-items-center rounded-md bg-control">
    <Plus size={12} stroke-width={2.5} />
  </button>
);

export const MinusButton = (p: { label: string; onClick: () => void }) => (
  <button onClick={() => p.onClick()} aria-label={p.label} class="grid size-[18px] shrink-0 place-items-center rounded-[5px] bg-control">
    <Minus size={10} stroke-width={2.5} />
  </button>
);

/** Small pill label; `accent` is the tinted status pill (like a "Pro" badge). */
export const Badge = (p: { accent?: boolean; children: JSX.Element }) => (
  <span class="t-meta shrink-0 rounded-full bg-selected px-2 leading-5" classList={{ "!bg-accent-soft !text-accent": p.accent }}>{p.children}</span>
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
