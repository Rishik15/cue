/**
 * Purpose: The guide page drawn over the note: what to type on a line, then the keys; after an update it opens with what's new.
 * Contents: Guide — the overlay, closed by any key or click; Cap — one keycap; Group — a titled block.
 */
import { For, Show } from "solid-js";
import { keys, NEW, examples } from "./guideRows";
import type { Options } from "./types";

const Cap = (p: { k: string }) => (
  <span class="t-tag grid h-5 min-w-5 shrink-0 place-items-center rounded border border-line bg-selected px-1">{p.k}</span>
);

const Group = (p: { title: string; hint?: string; children: unknown }) => (
  <section class="mb-4 min-w-0">
    <h3 class="t-tag text-muted">{p.title}</h3>
    <Show when={p.hint}><p class="mb-1 text-[12px] text-muted">{p.hint}</p></Show>
    {p.children as never}
  </section>
);

export function Guide(p: { kind: "welcome" | "help"; options: Options; welcome: string | null; onClose: () => void }) {
  const close = (e: Event) => { e.stopPropagation(); p.onClose(); };
  const update = () => (p.kind === "welcome" && p.welcome && p.welcome !== "first" ? p.welcome : null);
  return (
    <div
      ref={(el) => queueMicrotask(() => el.focus())}
      tabindex="-1"
      class="fade-in absolute inset-0 z-10 outline-none"
      style={{ "background-color": "var(--bg)", color: "var(--ink)" }}
      onKeyDown={close}
      onClick={close}
    >
      <div class="note-fade h-full overflow-y-auto px-10 py-5">
        <h2 class="mb-3 truncate text-[13px] font-semibold">Quick notes</h2>
        <Show when={update()}>
          {(v) => (
            <div class="mb-3">
              <p class="t-tag mb-1 truncate text-muted">What's new in {v()}</p>
              <For each={NEW}>{(n) => <p class="truncate text-[12px]" title={n}>{n}</p>}</For>
            </div>
          )}
        </Show>
        <Group title="Type on any line" hint="Plain text; the result appears as you type.">
          <For each={examples}>
            {(t) => (
              <div class="mb-1.5 min-w-0">
                <div class="w-fit max-w-full truncate rounded bg-selected px-1.5 text-[12px]" title={t.type}>{t.type}</div>
                <div class="truncate text-[12px] text-muted" title={t.does}>{t.does}</div>
              </div>
            )}
          </For>
        </Group>
        <Group title="Keys">
          <For each={keys}>
            {(r) => (
              <div class="flex h-6 min-w-0 items-center justify-between gap-3 text-[12px]">
                <span class="min-w-0 truncate" title={r.label}>{r.label}</span>
                <span class="flex shrink-0 gap-1"><For each={r.keys}>{(k) => <Cap k={k} />}</For></span>
              </div>
            )}
          </For>
        </Group>
        <Show when={p.kind === "welcome"}><p class="t-tag mt-2 text-muted">Press any key to start</p></Show>
      </div>
    </div>
  );
}
