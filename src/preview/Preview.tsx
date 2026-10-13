/**
 * Purpose: The preview window, laid out like a launcher form on an acrylic backdrop: a back arrow, a large Result block on the left and, on the
 * right, the Original text and two read-only fields (the action and its steps), then a footer with the action's name and its one key, Paste.
 * It appears the moment the shortcut is pressed (without taking the keyboard) showing "Reading the selection…", then the working state, then the
 * result. With several matching actions it lists them instead (Up and Down, Enter runs the highlighted one). Enter pastes, Escape or the back
 * arrow cancels and stops the work.
 * Contents: Preview — the window; Block — a label, a scrollable block and a note; Picker — the list of actions to choose from; Working — the
 * spinner shown in a block; Footer — the action name and keys; useView — the pending text from Rust, refreshed when it changes, which also shows the window.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LoaderCircle } from "lucide-solid";
import { createSignal, For, onCleanup, onMount, Show, type JSX } from "solid-js";
import { ActionDivider, ActionHint, BackButton, ValueField } from "../ui/kit";
import { NavIcon } from "../ui/NavIcon";
import { Text } from "../ui/glyphs";

type Choice = { at: number; name: string; steps: string };
type View = { original: string; result: string | null; via: string; steps: string; choices: Choice[] | null };

function useView() {
  const [view, setView] = createSignal<View | null>(null);
  const load = () => invoke<View | null>("preview_get").then(setView, () => setView(null));
  onMount(() => {
    load();
    const unlisten = listen("preview-changed", load);
    onCleanup(() => unlisten.then((f) => f()));
    // Shown as soon as the page is up, before the text exists; the window cannot take the focus until Rust says the text is copied.
    getCurrentWindow().show();
  });
  return view;
}

const words = (text: string) => text.trim().split(/\s+/).filter(Boolean).length;

/** A label over a block, with a one-line note under it (the reference form's helper text). The block's size is the column's, never the text's. */
function Block(p: { label: string; note?: string; muted?: boolean; children?: JSX.Element }) {
  return (
    <section class="flex min-h-0 min-w-0 flex-1 flex-col gap-2">
      <h2 class="t-label">{p.label}</h2>
      <div
        class="t-label min-h-0 flex-1 overflow-auto rounded-xl border border-line-strong bg-field px-3 py-2.5 select-text"
        classList={{ "!text-muted": p.muted }}
      >
        {p.children}
      </div>
      <p class="t-hint min-h-4 px-0.5">{p.note}</p>
    </section>
  );
}

const Body = (p: { text: string }) => <pre class="whitespace-pre-wrap break-words font-[inherit]">{p.text || "Empty"}</pre>;

const Working = (p: { label: string }) => (
  <div class="flex h-full items-center justify-center gap-2 text-muted" role="status">
    <LoaderCircle size={16} class="spin" />
    <span class="t-label !text-muted">{p.label}</span>
  </div>
);

function Picker(p: { choices: Choice[]; active: number; onPick: (at: number) => void }) {
  return (
    <Block label="Choose an Action">
      <div class="-mx-2 -my-1 flex flex-col gap-px" role="listbox" aria-label="Actions">
        <For each={p.choices}>
          {(c, i) => (
            <button
              role="option"
              aria-selected={i() === p.active}
              onClick={() => p.onPick(c.at)}
              class="flex min-h-9 w-full min-w-0 flex-col items-start justify-center rounded-lg px-2 py-1 text-left"
              classList={{ "bg-selected": i() === p.active }}
              title={c.name}
            >
              <span class="t-label w-full truncate">{c.name}</span>
              <span class="t-hint w-full truncate">{c.steps}</span>
            </button>
          )}
        </For>
      </div>
    </Block>
  );
}

function Footer(p: { name: string; state: "reading" | "choosing" | "working" | "ready"; act: (command: string) => void; run: () => void }) {
  const cancel = <ActionHint label="Cancel" keys={["Esc"]} onClick={() => p.act("preview_close")} />;
  return (
    <footer class="flex h-10 shrink-0 items-center justify-between gap-4 border-t border-line pl-[13px] pr-[11px]">
      <span class="flex min-w-0 items-center gap-2" title={p.name}>
        <NavIcon icon={Text} size={18} />
        <span class="t-label truncate">{p.name}</span>
      </span>
      <div class="flex shrink-0 items-center">
        <Show when={p.state === "ready"}><ActionHint label="Paste" keys={["↵"]} onClick={() => p.act("preview_apply")} /></Show>
        <Show when={p.state === "choosing"}>
          <ActionHint label="Run" keys={["↵"]} onClick={p.run} />
          <ActionDivider />
        </Show>
        <Show when={p.state !== "ready"}>{cancel}</Show>
      </div>
    </footer>
  );
}

/** Original and the read-only facts about the action, stacked in the right column. */
function Details(p: { view: View | null }) {
  return (
    <div class="flex min-h-0 min-w-0 flex-col gap-1">
      <Block label="Original" note={p.view ? `${words(p.view.original)} words` : undefined} muted>
        <Show when={p.view} fallback={<Working label="Reading the selection…" />}>{(v) => <Body text={v().original} />}</Show>
      </Block>
      <ValueField label="Action" value={p.view?.via ?? ""} />
      <Show when={p.view?.steps}><ValueField label="Steps" value={p.view?.steps ?? ""} /></Show>
    </div>
  );
}

/** The picker's highlight and its actions, and the window's keys: Escape cancels, Up and Down move, Enter runs the highlighted action or pastes. */
function useKeys(view: () => View | null, act: (command: string) => void) {
  const [active, setActive] = createSignal(0);
  const choices = () => view()?.choices ?? null;
  const pick = (at: number) => invoke("preview_choose", { at });
  const run = () => { const c = choices()?.[active()]; if (c) pick(c.at); };
  const move = (by: number) => setActive(Math.max(0, Math.min((choices()?.length ?? 1) - 1, active() + by)));
  const keys = (e: KeyboardEvent) => {
    if (e.key === "Escape") act("preview_close");
    if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); move(e.key === "ArrowDown" ? 1 : -1); }
    if (e.key === "Enter") { if (choices()) run(); else act("preview_apply"); }
  };
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return { active, choices, pick, run };
}

/** The two columns: Result beside Original and the action's facts, or Original beside the list when there is a choice to make. */
function Columns(p: { view: View | null; active: number; onPick: (at: number) => void }) {
  const result = () => p.view?.result ?? null;
  return (
    <Show
      when={p.view?.choices}
      fallback={
        <>
          <Block label="Result" note={result() ? `${words(result() ?? "")} words` : undefined}>
            <Show when={result() !== null} fallback={<Working label={p.view ? "Working on it…" : "Reading the selection…"} />}>
              <Body text={result() ?? ""} />
            </Show>
          </Block>
          <Details view={p.view} />
        </>
      }
    >
      {(list) => (
        <>
          <Block label="Original" note={`${words(p.view?.original ?? "")} words`} muted><Body text={p.view?.original ?? ""} /></Block>
          <Picker choices={list()} active={p.active} onPick={p.onPick} />
        </>
      )}
    </Show>
  );
}

export function Preview() {
  const view = useView();
  const act = (command: string) => invoke(command);
  const { active, choices, pick, run } = useKeys(view, act);
  const state = () => (!view() ? "reading" : choices() ? "choosing" : view()?.result === null ? "working" : "ready");
  return (
    <div class="flex h-full flex-col bg-card/55">
      <header data-tauri-drag-region class="h-[62px] shrink-0 pl-[17px] pt-[22px]">
        <BackButton onClick={() => act("preview_close")} />
      </header>
      <main class="grid min-h-0 flex-1 grid-cols-2 gap-6 px-14 pb-3">
        <Columns view={view()} active={active()} onPick={pick} />
      </main>
      <Footer name={view()?.via || (choices() ? "Choose an action" : "Cue")} state={state()} act={act} run={run} />
    </div>
  );
}
