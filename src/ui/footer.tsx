/**
 * Purpose: Text actions with key caps, as in a modal's footer (used by the preview window).
 * Contents: Kbd — one key cap; ActionHint — a text action followed by its caps; ActionDivider — the hairline between two actions.
 */
import { For, Show, type JSX } from "solid-js";

/** A key cap such as Ctrl, K or ↵ (18 high, as small as the reference footer's): equal height for every cap, a hairline border, caps of one combination 4 apart. */
export const Kbd = (p: { children: JSX.Element }) => (
  <kbd class="t-hint grid h-[18px] min-w-[18px] place-items-center rounded-[5px] border border-line px-1 font-[inherit] leading-none">{p.children}</kbd>
);

/** A text action followed by its keys. `muted` is for the secondary action, `danger` for a destructive one. */
export const ActionHint = (p: { label: string; keys?: string[]; onClick: () => void; muted?: boolean; danger?: boolean }) => (
  <button onClick={() => p.onClick()} class="t-label flex h-7 items-center gap-2 rounded-md px-2" classList={{ "!text-muted": p.muted, "!text-danger": p.danger }}>
    {p.label}
    <Show when={p.keys?.length}><span class="flex gap-1"><For each={p.keys}>{(k) => <Kbd>{k}</Kbd>}</For></span></Show>
  </button>
);

export const ActionDivider = () => <span class="mx-1 h-4 w-px bg-line" aria-hidden="true" />;
