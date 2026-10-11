/**
 * Purpose: The public face of the UI kit: page layout and rows here, controls in `controls`, the footer in `footer`, the banner in `banner`.
 * Contents: Page, Section, Card, Row, ListRow (truncating title and subtitle, optional click), AddRow, BackButton, Button, Progress, IconButton,
 * Badge, Meter (1 to 5 segments); re-exports of the controls, footer and banner so pages import from one place.
 */
import { ArrowLeft, CirclePlus } from "lucide-solid";
import { For, type JSX } from "solid-js";

export * from "./controls";
export * from "./footer";
export * from "./banner";

/** The page's padding leaves room under the last card, so nothing sits against the footer or the window edge. */
export const Page = (p: { children: JSX.Element }) => (
  <div class="flex flex-col gap-6 px-4 pb-6 pt-4">{p.children}</div>
);

export const Section = (p: { title?: string; children: JSX.Element }) => (
  <section class="flex flex-col gap-2">
    {p.title && <h2 class="t-meta px-3 pb-0.5">{p.title}</h2>}
    {p.children}
  </section>
);

export const Card = (p: { children: JSX.Element }) => (
  <div class="divide-y divide-line overflow-hidden rounded-[10px] bg-card">{p.children}</div>
);

/** `badge` is a small status pill after the label ("Experimental"). */
export const Row = (p: { label: string; hint?: string; badge?: string; children: JSX.Element }) => (
  <div class="flex items-center justify-between gap-6 px-3 py-[13px]">
    <div class="flex min-w-0 flex-col gap-0.5">
      <div class="t-label flex items-center gap-2">{p.label}{p.badge && <Badge warn>{p.badge}</Badge>}</div>
      {p.hint && <div class="t-hint max-w-[52ch]">{p.hint}</div>}
    </div>
    <div class="shrink-0">{p.children}</div>
  </div>
);

/** One entry of a list card: a title over a one-line subtitle (both truncate), an optional click target and a trailing control. */
export const ListRow = (p: { title: string; subtitle?: string; onClick?: () => void; children?: JSX.Element }) => (
  <div class="flex items-center">
    <button disabled={!p.onClick} onClick={() => p.onClick?.()} class="flex min-w-0 flex-1 flex-col gap-0.5 py-[13px] pl-3 text-left disabled:cursor-default">
      <span class="t-label truncate">{p.title}</span>
      {p.subtitle && <span class="t-hint truncate">{p.subtitle}</span>}
    </button>
    {p.children && <div class="shrink-0 py-[13px] pl-6 pr-3">{p.children}</div>}
  </div>
);

/** The last row of a list card: adds an entry. The whole row is the click target. */
export const AddRow = (p: { label: string; onClick: () => void }) => (
  <button onClick={() => p.onClick()} class="t-label flex h-11 w-full items-center gap-3 px-3 text-left">
    <CirclePlus size={18} stroke-width={1.5} class="text-muted" />
    {p.label}
  </button>
);

/** Back to the previous view in a modal form, as in Raycast's Create Snippet: a small bordered square at the top left. */
export const BackButton = (p: { onClick: () => void }) => (
  <button onClick={() => p.onClick()} aria-label="Back" class="grid size-7 shrink-0 place-items-center rounded-md border border-line bg-control">
    <ArrowLeft size={16} stroke-width={2} />
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

/** Small pill label; `accent` and `warn` are the tinted status pills (Raycast's "Pro" and "Experimental"). */
export const Badge = (p: { accent?: boolean; warn?: boolean; children: JSX.Element }) => (
  <span class="t-meta shrink-0 rounded-full bg-selected px-2 leading-5" classList={{ "!bg-accent/15 !text-accent": p.accent, "!bg-amber-400/15 !text-amber-400": p.warn }}>{p.children}</span>
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
