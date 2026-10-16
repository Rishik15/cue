/**
 * Purpose: The sidebar's page glyphs: Tabler's outline icons used as they are, in the text colour, all on the same 24 unit grid and stroke so they sit level in their tiles.
 * Contents: Gear, Keys, Chip, Mic, Text, Scan, Snippet, Book — one Tabler icon per page; Ball — the cue ball mark for the About tile; CueLogo — the full app icon (top of the About page); each takes a pixel `size`.
 */
import { IconBook, IconBrackets, IconCommand, IconCpu, IconMicrophone, IconScan, IconSettings, IconTypography } from "@tabler/icons-solidjs";
import type { Component, JSX } from "solid-js";

type Props = { size?: number };

const STROKE = "2";

/** A Tabler icon as a glyph: the one size and stroke every icon in the app shares, so they sit level. */
export const fromTabler = (Icon: Component<{ size?: number; stroke?: string }>) => (p: Props) => <Icon size={p.size ?? 16} stroke={STROKE} />;

export const Gear = fromTabler(IconSettings);
export const Keys = fromTabler(IconCommand);
export const Chip = fromTabler(IconCpu);
export const Mic = fromTabler(IconMicrophone);
export const Text = fromTabler(IconTypography);
export const Scan = fromTabler(IconScan);
export const Snippet = fromTabler(IconBrackets);
export const Book = fromTabler(IconBook);

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
