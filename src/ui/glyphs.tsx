/**
 * Purpose: The sidebar's page glyphs: Lucide's outline icons at one stroke, in the text colour, all on the same 24 unit grid and stroke so they sit level in their tiles.
 * Contents: Gear, Keys, Chip, Mic, Text, Scan, Snippet, Note, Book — one Lucide icon per page; Ball — the cue ball mark for the About tile; CueLogo — the full app icon (top of the About page); each takes a pixel `size`.
 */
import { Braces, Command, Cpu, Languages, Mic as MicIcon, Scan as ScanIcon, Settings, StickyNote, Type, type LucideProps } from "lucide-solid";
import type { Component, JSX } from "solid-js";

type Props = { size?: number };

/** A Lucide icon as a glyph: the one size and stroke every icon in the app shares, so they sit level. */
const glyph = (Icon: Component<LucideProps>) => (p: Props) => <Icon size={p.size ?? 16} stroke-width={2} />;

export const Gear = glyph(Settings);
export const Keys = glyph(Command);
export const Chip = glyph(Cpu);
export const Mic = glyph(MicIcon);
export const Text = glyph(Type);
export const Scan = glyph(ScanIcon);
export const Snippet = glyph(Braces);
export const Note = glyph(StickyNote);
export const Book = glyph(Languages);

function Svg(p: Props & { children: JSX.Element }) {
  return (
    <svg width={p.size ?? 16} height={p.size ?? 16} viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      {p.children}
    </svg>
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
