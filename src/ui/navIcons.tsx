/**
 * Purpose: The sidebar's page icons, drawn in layers like Raycast's: a solid front shape over lighter back shapes (the same colour at lower
 * opacity), so each glyph has depth when it sits on the lit tile of `NavIcon`. They take their colour from the text colour.
 * Contents: GeneralIcon (gear), DictationIcon (microphone with sound bars), ModelsIcon (chip), RulesIcon (two linked blocks), AboutIcon (info).
 */
import { For, type JSX } from "solid-js";
import type { Glyph } from "./NavIcon";

const Svg = (p: { size?: number; children: JSX.Element }) => (
  <svg
    width={p.size ?? 16}
    height={p.size ?? 16}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    {p.children}
  </svg>
);

const TEETH = [0, 45, 90, 135, 180, 225, 270, 315];

export const GeneralIcon: Glyph = (p) => (
  <Svg size={p.size}>
    <For each={TEETH}>{(a) => <line x1="12" y1="1.8" x2="12" y2="4.2" stroke-width="2.4" transform={`rotate(${a} 12 12)`} opacity="0.55" />}</For>
    <circle cx="12" cy="12" r="6.6" fill="currentColor" fill-opacity="0.22" />
    <circle cx="12" cy="12" r="2.6" fill="currentColor" stroke="none" />
  </Svg>
);

export const DictationIcon: Glyph = (p) => (
  <Svg size={p.size}>
    <g opacity="0.45">
      <line x1="2.5" y1="10" x2="2.5" y2="14" />
      <line x1="5.5" y1="8" x2="5.5" y2="16" />
      <line x1="18.5" y1="8" x2="18.5" y2="16" />
      <line x1="21.5" y1="10" x2="21.5" y2="14" />
    </g>
    <rect x="9" y="2.5" width="6" height="12" rx="3" fill="currentColor" stroke="none" />
    <path d="M7 11.5a5 5 0 0 0 10 0" />
    <line x1="12" y1="16.7" x2="12" y2="21" />
    <line x1="8.5" y1="21" x2="15.5" y2="21" />
  </Svg>
);

export const ModelsIcon: Glyph = (p) => (
  <Svg size={p.size}>
    <g stroke-width="1.8" opacity="0.55">
      <path d="M9 2.2v2.4M12 2.2v2.4M15 2.2v2.4M9 19.4v2.4M12 19.4v2.4M15 19.4v2.4" />
      <path d="M2.2 9h2.4M2.2 12h2.4M2.2 15h2.4M19.4 9h2.4M19.4 12h2.4M19.4 15h2.4" />
    </g>
    <rect x="5" y="5" width="14" height="14" rx="3" fill="currentColor" fill-opacity="0.2" />
    <rect x="9" y="9" width="6" height="6" rx="1.5" fill="currentColor" stroke="none" />
  </Svg>
);

export const RulesIcon: Glyph = (p) => (
  <Svg size={p.size}>
    <path d="M10.5 8h2.5a3 3 0 0 1 3 3v2" opacity="0.6" />
    <rect x="2.5" y="3.5" width="9" height="9" rx="2.5" fill="currentColor" stroke="none" />
    <rect x="11.5" y="12.5" width="10" height="8.5" rx="2.5" fill="currentColor" fill-opacity="0.22" />
  </Svg>
);

export const AboutIcon: Glyph = (p) => (
  <Svg size={p.size}>
    <circle cx="12" cy="12" r="9.5" opacity="0.5" />
    <circle cx="12" cy="12" r="6.8" fill="currentColor" fill-opacity="0.2" stroke="none" />
    <circle cx="12" cy="7.9" r="1.2" fill="currentColor" stroke="none" />
    <line x1="12" y1="11" x2="12" y2="16.5" stroke-width="2.3" />
  </Svg>
);
