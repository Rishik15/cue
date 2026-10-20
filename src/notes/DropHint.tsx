/**
 * Purpose: The cover shown over the note while a picture is dragged in (a picture icon) or being read (the loading bars).
 * Contents: DropHint — the cover, for the "over" and "reading" states of `useImageDrop`.
 */
import { Image } from "lucide-solid";
import { For, Show } from "solid-js";
import type { Drop } from "./useImageDrop";

export const DropHint = (p: { drop: Drop }) => (
  <Show when={p.drop !== "idle"}>
    <div class="fade-in absolute inset-0 z-10 flex items-center justify-center bg-[color-mix(in_srgb,var(--bg)_88%,transparent)] text-muted" role="status" aria-label={p.drop === "over" ? "Drop a picture to read its text" : "Reading the picture"}>
      <Show when={p.drop === "over"} fallback={
        <span class="flex h-4 w-[27px] items-center justify-between" aria-hidden="true">
          <For each={[0, 1, 2, 3, 4]}>{(i) => <span class="wave-bar" style={{ "animation-delay": `${i * 120}ms` }} />}</For>
        </span>
      }>
        <Image size={32} strokeWidth={1.5} aria-hidden="true" />
      </Show>
    </div>
  </Show>
);
