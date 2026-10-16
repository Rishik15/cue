/**
 * Purpose: A hand-drawn note on the setup pages: a small, almost straight arrow with a slight hand-drawn sway that points up at the Download button, and a handwritten line right under it.
 * The arrow is drawn like an ink doodle: one slightly swaying line, a faint second stroke a hair off the first, round caps and joins, and a head made of two wings of slightly different length.
 * Both sit under the button: the arrow's tip is centred on the button, the text ends where the button ends. The note is pulled up over the card's bottom edge so the tip lands close to the button, and ignores the pointer so it never blocks a click.
 * Contents: Doodle — the note; it only hides (never leaves the layout) once there is nothing left to download, so the page does not shift.
 */
const SHAFT = "M23 56C20 42 25 24 22 7";
const HEAD = "M14 17Q18 12 22 6Q26 11 31 14";

export function Doodle(p: { text: string; show: boolean }) {
  return (
    <div class="pointer-events-none -mt-6 flex h-[84px] flex-col items-end" classList={{ invisible: !p.show }} aria-hidden={!p.show}>
      <svg width="40" height="60" viewBox="0 0 40 60" fill="none" stroke="var(--accent)" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class="mr-[36px] shrink-0">
        <g stroke-width="1.8">
          <path d={SHAFT} />
          <path d={HEAD} />
        </g>
        <g stroke-width="1.2" opacity="0.4" transform="translate(0.9 0.7)">
          <path d={SHAFT} />
          <path d={HEAD} />
        </g>
      </svg>
      <span class="t-note mr-3 -rotate-2">{p.text}</span>
    </div>
  );
}
