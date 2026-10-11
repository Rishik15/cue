/**
 * Purpose: The sidebar's page glyphs, drawn solid and plain on a 24 unit grid in one colour (`currentColor`), like Raycast's settings icons.
 * Contents: Gear, Mic, Chip, Flow — one glyph per page; Ball — the cue ball mark for the About tile; CueLogo — the full app icon (top of the About page); each takes a pixel `size`.
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
      <rect x="10" y="2.5" width="7" height="12" rx="3.5" stroke="none" />
      <path d="M7.5 11.5a6.5 6.5 0 0 0 13 0" fill="none" stroke-width="2" />
      <path d="M14 18v3.2M10.5 21.2h7" fill="none" stroke-width="2" />
      <path d="M2.5 6h4.5M2.5 10h3M2.5 14h4.5" fill="none" stroke-width="1.8" opacity="0.7" />
    </Svg>
  );
}

export function Chip(p: Props) {
  return (
    <Svg {...p}>
      <g fill="none" stroke-width="1.7">
        <rect x="5.5" y="5.5" width="13" height="13" rx="2.2" />
        <rect x="8.7" y="8.7" width="6.6" height="6.6" rx="0.4" />
        <path d="M9 2.5v3M12 2.5v3M15 2.5v3M9 18.5v3M12 18.5v3M15 18.5v3M2.5 9h3M2.5 12h3M2.5 15h3M18.5 9h3M18.5 12h3M18.5 15h3" />
      </g>
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

/** The cue ball in one colour: a disc with the red dot cut out, for the sidebar tile. */
export function Ball(p: Props) {
  return (
    <Svg {...p}>
      <path fill-rule="evenodd" stroke="none" d="M12 2.5a9.5 9.5 0 1 0 0 19 9.5 9.5 0 0 0 0-19Zm3.2 3.4a2.2 2.2 0 1 1 0 4.4 2.2 2.2 0 0 1 0-4.4Z" />
    </Svg>
  );
}

/** The app icon, the same drawing as `assets/icon.svg` cropped to the tile. */
export function CueLogo(p: Props) {
  return (
    <svg width={p.size ?? 16} height={p.size ?? 16} viewBox="100 100 824 824" aria-hidden="true">
      <rect x="100" y="100" width="824" height="824" rx="185" fill="#0E5A43" />
      <circle cx="552" cy="552" r="250" fill="#0A4634" />
      <circle cx="512" cy="512" r="250" fill="#F6F3EC" />
      <path d="M672.05 319.95A250 250 0 1 1 319.95 672.05A250 250 0 0 0 672.05 319.95Z" fill="#E1DCD0" />
      <circle cx="590" cy="432" r="58" fill="#D7263D" />
    </svg>
  );
}
