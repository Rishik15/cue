/**
 * Purpose: The footer bar of a form page and its text actions with key caps, as in Raycast's modals (docs/design/raycast-portfolio.md).
 * Contents: Kbd — one key cap; ActionHint — a text action followed by its caps; ActionDivider — the hairline between two actions;
 * FooterBar — renders its children in the footer slot under the page's scroll area (set by `App` through `setFooterSlot`), edge to edge.
 */
import { createSignal, For, Show, type JSX } from "solid-js";
import { Portal } from "solid-js/web";

const [footerSlot, setFooterSlot] = createSignal<HTMLElement>();
export { setFooterSlot };

/** A key cap such as Ctrl, K or ↵: equal height for every cap, a hairline border, caps of one combination 4 apart. */
export const Kbd = (p: { children: JSX.Element }) => (
  <kbd class="t-hint grid h-6 min-w-6 place-items-center rounded-[5px] border border-line px-1.5 font-[inherit] leading-none">{p.children}</kbd>
);

/** A text action followed by its keys. `muted` is for the secondary action, `danger` for a destructive one. */
export const ActionHint = (p: { label: string; keys?: string[]; onClick: () => void; muted?: boolean; danger?: boolean }) => (
  <button onClick={() => p.onClick()} class="t-label flex h-8 items-center gap-2.5 rounded-md px-2" classList={{ "!text-muted": p.muted, "!text-danger": p.danger }}>
    {p.label}
    <Show when={p.keys?.length}><span class="flex gap-1"><For each={p.keys}>{(k) => <Kbd>{k}</Kbd>}</For></span></Show>
  </button>
);

export const ActionDivider = () => <span class="mx-1 h-4 w-px bg-line" aria-hidden="true" />;

/** Fixed 48 high, hairline above, the full width of the content column; the page above scrolls on its own. */
export const FooterBar = (p: { children: JSX.Element }) => (
  <Show when={footerSlot()}>
    {(slot) => (
      <Portal mount={slot()}>
        <div class="flex h-12 items-center justify-between border-t border-line bg-bg px-4">{p.children}</div>
      </Portal>
    )}
  </Show>
);
