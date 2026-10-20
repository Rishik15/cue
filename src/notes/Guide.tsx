/**
 * Purpose: The guide page drawn over the note: the welcome (first open, after an update) and the key help.
 * Contents: Guide — the overlay, closed by any key or click; Cap — one keycap; Group — a titled list of rows.
 */
import { For, Show } from "solid-js";
import { sections, type Section } from "./guideRows";
import type { Options } from "./types";

const NEW = ["Notes palette: Ctrl+P", "Top bar and titles", "Find and replace bar", "Key help: Ctrl+/"];

const Cap = (p: { k: string }) => (
  <span class="t-tag grid h-5 min-w-5 shrink-0 place-items-center rounded border border-line bg-selected px-1">{p.k}</span>
);

function Group(p: { section: Section }) {
  return (
    <section class="mb-3 min-w-0">
      <h3 class="t-tag mb-1 text-muted">{p.section.title}</h3>
      <For each={p.section.rows}>
        {(r) => (
          <div class="flex h-6 min-w-0 items-center justify-between gap-3 text-[12px]">
            <span class="min-w-0 truncate" title={r.label}>{r.label}</span>
            <span class="flex shrink-0 gap-1"><For each={r.keys}>{(k) => <Cap k={k} />}</For></span>
          </div>
        )}
      </For>
    </section>
  );
}

export function Guide(p: { kind: "welcome" | "help"; options: Options; welcome: string | null; onClose: () => void }) {
  const close = (e: Event) => { e.stopPropagation(); p.onClose(); };
  const update = () => (p.kind === "welcome" && p.welcome && p.welcome !== "first" ? p.welcome : null);
  return (
    <div
      ref={(el) => queueMicrotask(() => el.focus())}
      tabindex="-1"
      class="fade-in absolute inset-0 z-10 overflow-y-auto px-10 py-5 outline-none"
      style={{ "background-color": "var(--bg)", color: "var(--ink)" }}
      onKeyDown={close}
      onClick={close}
    >
      <Show when={p.kind === "welcome"}><h2 class="mb-3 truncate text-[13px] font-semibold">Quick notes</h2></Show>
      <Show when={update()}>
        {(v) => (
          <div class="mb-3">
            <p class="t-tag mb-1 truncate text-muted">What's new in {v()}</p>
            <For each={NEW}>{(n) => <p class="truncate text-[12px]" title={n}>{n}</p>}</For>
          </div>
        )}
      </Show>
      <div><For each={sections(p.kind)}>{(s) => <Group section={s} />}</For></div>
      <Show when={p.kind === "welcome"}><p class="t-tag mt-2 text-muted">Press any key to start</p></Show>
    </div>
  );
}
