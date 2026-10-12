/**
 * Purpose: The statistics cards of the Dictation page, (docs/design/reference-portfolio.md).
 * Contents: StatsCard — equal columns of a label over a big light number with an optional unit, split by short hairlines; Heatmap — one cell
 * per day in weekly columns (Sunday at the top) with month labels, shaded by how much was dictated that day.
 */
import { For, Show } from "solid-js";

export type StatItem = { label: string; value: string; unit?: string };

export function StatsCard(p: { items: StatItem[] }) {
  return (
    <div class="grid h-32 rounded-[18px] border border-line-strong bg-field" style={{ "grid-template-columns": `repeat(${p.items.length}, minmax(0, 1fr))` }}>
      <For each={p.items}>
        {(item, i) => (
          <div class="relative flex min-w-0 flex-col items-center justify-center gap-0.5 px-2">
            <span class="t-label text-muted">{item.label}</span>
            <span class="flex items-baseline gap-1">
              <span class="t-number">{item.value}</span>
              <Show when={item.unit}><span class="t-number-unit">{item.unit}</span></Show>
            </span>
            <Show when={i() > 0}><span class="absolute bottom-4 left-0 top-4 w-px bg-line" /></Show>
          </div>
        )}
      </For>
    </div>
  );
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const SHADES = [0, 30, 50, 75, 100];

/** 0 for a quiet day, 1 to 4 relative to the busiest day. */
const level = (n: number, max: number) => (n <= 0 ? 0 : Math.min(4, Math.ceil((4 * n) / max)));

/** `days` is oldest first and ends today; the first column is padded so each row is one weekday. */
export function Heatmap(p: { days: number[]; today: Date }) {
  const first = () => { const d = new Date(p.today); d.setDate(d.getDate() - (p.days.length - 1)); return d; };
  const cells = () => [...Array<null>(first().getDay()).fill(null), ...p.days];
  const columns = () => Math.ceil(cells().length / 7);
  const max = () => Math.max(1, ...p.days);
  /** A month name above the column that holds the 1st of that month (a month already under way at the left edge has no name). */
  const labelAt = (col: number) => {
    for (let d = 0; d < 7; d++) {
      const day = new Date(first());
      day.setDate(day.getDate() + col * 7 + d - first().getDay());
      if (day >= first() && day <= p.today && day.getDate() === 1) return MONTHS[day.getMonth()];
    }
    return "";
  };
  const grid = () => ({ "grid-template-columns": `repeat(${columns()}, minmax(0, 1fr))` });
  return (
    <div class="flex h-32 flex-col gap-2 rounded-[18px] border border-line-strong bg-field p-3.5" role="img" aria-label="Words dictated per day over the last year">
      <div class="grid gap-[3px]" style={grid()}>
        <For each={Array.from({ length: columns() }, (_, c) => labelAt(c))}>{(m) => <span class="t-micro whitespace-nowrap">{m}</span>}</For>
      </div>
      <div class="grid min-h-0 flex-1 grid-flow-col grid-rows-7 gap-[3px]" style={grid()}>
        <For each={cells()}>
          {(n) => (
            <span
              class="rounded-[2.5px]"
              classList={{ invisible: n === null }}
              style={{ background: n && level(n, max()) > 0 ? `color-mix(in srgb, var(--accent) ${SHADES[level(n, max())]}%, transparent)` : "var(--selected)" }}
            />
          )}
        </For>
      </div>
    </div>
  );
}
