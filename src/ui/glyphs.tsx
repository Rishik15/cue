/**
 * Purpose: The sidebar's page glyphs, drawn solid and plain on a 24 unit grid in one colour (`currentColor`), like Raycast's settings icons.
 * Contents: Gear, Mic, Chip, Flow, Info — one glyph per page; each takes a pixel `size`.
 */
import { For, type JSX } from "solid-js";

type Props = { size?: number };

function Svg(p: Props & { children: JSX.Element }) {
  return (
    <svg width={p.size ?? 16} height={p.size ?? 16} viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      {p.children}
    </svg>
  );
}

/** Eight teeth as short thick lines from the ring outwards, with coordinates worked out here so no renderer has to rotate anything. */
const TEETH = [0, 1, 2, 3, 4, 5, 6, 7].map((i) => {
  const [cos, sin] = [Math.cos((i * Math.PI) / 4), Math.sin((i * Math.PI) / 4)];
  const at = (r: number) => ({ x: +(12 + r * cos).toFixed(2), y: +(12 + r * sin).toFixed(2) });
  return { from: at(6.4), to: at(10.6) };
});

export function Gear(p: Props) {
  return (
    <Svg {...p}>
      <circle cx="12" cy="12" r="5.6" fill="none" stroke-width="3.6" />
      <For each={TEETH}>{(t) => <line x1={t.from.x} y1={t.from.y} x2={t.to.x} y2={t.to.y} stroke-width="3.2" stroke-linecap="butt" />}</For>
    </Svg>
  );
}

export function Mic(p: Props) {
  return (
    <Svg {...p}>
      <rect x="8.5" y="2.5" width="7" height="12" rx="3.5" stroke="none" />
      <path d="M5.5 11.5a6.5 6.5 0 0 0 13 0" fill="none" stroke-width="2" />
      <path d="M12 18v3.2M8.5 21.2h7" fill="none" stroke-width="2" />
    </Svg>
  );
}

export function Chip(p: Props) {
  return (
    <Svg {...p}>
      <path fill-rule="evenodd" stroke="none" d="M7.5 5h9A2.5 2.5 0 0 1 19 7.5v9a2.5 2.5 0 0 1-2.5 2.5h-9A2.5 2.5 0 0 1 5 16.5v-9A2.5 2.5 0 0 1 7.5 5Zm1.5 4.5v5h6v-5H9Z" />
      <path d="M9.5 2.2v2M14.5 2.2v2M9.5 19.8v2M14.5 19.8v2M2.2 9.5h2M2.2 14.5h2M19.8 9.5h2M19.8 14.5h2" fill="none" stroke-width="2" />
    </Svg>
  );
}

export function Flow(p: Props) {
  return (
    <Svg {...p}>
      <rect x="3" y="3" width="9" height="9" rx="2.5" stroke="none" />
      <rect x="14" y="14" width="7" height="7" rx="2" stroke="none" />
      <path d="M7.5 12v2.5a2.5 2.5 0 0 0 2.5 2.5h2.5" fill="none" stroke-width="2" />
    </Svg>
  );
}

export function Info(p: Props) {
  return (
    <Svg {...p}>
      <path fill-rule="evenodd" stroke="none" d="M12 2.5a9.5 9.5 0 1 0 0 19 9.5 9.5 0 0 0 0-19Zm0 4.2a1.4 1.4 0 1 1 0 2.8 1.4 1.4 0 0 1 0-2.8Zm-1.3 4.6h2.6v6.2h-2.6v-6.2Z" />
    </Svg>
  );
}
