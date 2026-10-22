/**
 * Purpose: Settings window root: header, sidebar and the scrolling page.
 * Contents: App — layout (also follows Rust's request to open a page); NAV — page list; an item with `section` starts a new sidebar group (app settings, then one page per feature, then About); useScrollPerView — a page opens at its top, a sub-view too, and leaving a sub-view puts the list back where it was.
 */
import { LayoutGrid } from "lucide-solid";
import { createComputed, createEffect, createSignal, lazy, Match, on, onCleanup, onMount, Show, Switch } from "solid-js";
import { smoothScroll } from "./smoothScroll";
import { General } from "./pages/General";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { current, detailTitle, go } from "./nav";
import { Ball, Book, Chip, Gear, Keys, Mic, Note, Snippet, Text } from "./ui/glyphs";
import { Sidebar, type NavDef } from "./ui/Sidebar";
import { Onboarding } from "./onboarding/Onboarding";
import { useSetting } from "./settings";
import { Toaster } from "./ui/Toast";
import { watchUpdates } from "./useUpdate";
import { TitleBar } from "./ui/TitleBar";

// The first page is in the main bundle; the others load when opened (they are local files, so it takes a moment), which keeps their code out of the window's startup.
const [About, Dictation, Models, NotesPage, Shortcuts, Snippets, Words, TextPage] = [
  lazy(() => import("./pages/About").then((m) => ({ default: m.About }))),
  lazy(() => import("./pages/Dictation").then((m) => ({ default: m.Dictation }))),
  lazy(() => import("./pages/Models").then((m) => ({ default: m.Models }))),
  lazy(() => import("./pages/Notes").then((m) => ({ default: m.Notes }))),
  lazy(() => import("./pages/Shortcuts").then((m) => ({ default: m.Shortcuts }))),
  lazy(() => import("./pages/Snippets").then((m) => ({ default: m.Snippets }))),
  lazy(() => import("./pages/Words").then((m) => ({ default: m.Words }))),
  lazy(() => import("./pages/Text").then((m) => ({ default: m.Text }))),
];

// Dev only: the dynamic import keeps the Kit page out of the release bundle.
const Kit = import.meta.env.DEV ? lazy(() => import("./pages/Kit").then((m) => ({ default: m.Kit }))) : () => null;

const NAV: NavDef[] = [
  { id: "general", label: "General", icon: Gear },
  { id: "shortcuts", label: "Shortcuts", icon: Keys },
  { id: "models", label: "Models", icon: Chip },
  { id: "dictation", label: "Dictation", icon: Mic, section: true },
  { id: "text", label: "Text", icon: Text },
  { id: "snippets", label: "Snippets", icon: Snippet },
  { id: "notes", label: "Notes", icon: Note },
  { id: "words", label: "Dictionary", icon: Book },
  { id: "about", label: "About", icon: Ball, section: true },
  ...(import.meta.env.DEV ? [{ id: "kit", label: "Kit", icon: LayoutGrid }] : []),
];

/** The page name, or the name of the sub-view (the rule editor) when one is open. */
const headerTitle = () => (current().detail ? detailTitle() : (NAV.find((n) => n.id === current().page)?.label ?? ""));

/** Every page and every sub-view (an editor) opens at its top. Only going back from a sub-view puts its list where it was; that position is used once. */
function useScrollPerView(main: () => HTMLElement | undefined) {
  const saved = new Map<string, number>();
  const view = () => `${current().page}:${current().detail}`;
  // A computation runs before the page is redrawn, so the old view's scroll position is still readable.
  createComputed(on(view, (now, was) => {
    const page = main();
    const opensItsOwnSubView = was && now.endsWith(":true") && was.split(":")[0] === now.split(":")[0];
    if (page && was && opensItsOwnSubView) saved.set(was, page.scrollTop);
  }, { defer: true }));
  createEffect(on(view, (now) => {
    const page = main();
    if (page) page.scrollTop = saved.get(now) ?? 0;
    saved.delete(now);
  }, { defer: true }));
}

export default function App() {
  const [main, setMain] = createSignal<HTMLElement>();
  const [onboarded, setOnboarded] = useSetting("onboarded", false);
  useScrollPerView(main);
  // Lenis needs the content element to exist, so it starts after the first render.
  onMount(() => { const page = main(); if (page) onCleanup(smoothScroll(page)); });
  onMount(() => onCleanup(watchUpdates()));
  // Rust sends the window to a page (About, when an update waits): at creation it is asked for, afterwards it is told.
  onMount(() => {
    void invoke<string | null>("take_start_page").then((page) => page && go(page), () => undefined);
    const off = listen<string>("cue-goto", (e) => go(e.payload));
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  return (
    <>
    <Show when={onboarded()} fallback={<Onboarding onDone={() => setOnboarded(true)} />}>
    <div class="flex h-full flex-col bg-bg">
      <TitleBar title="Cue" page={headerTitle()} />
      <div class="flex min-h-0 flex-1">
        <Sidebar items={NAV} active={current().page} onSelect={go} />
        <div class="flex min-h-0 min-w-0 flex-1 flex-col">
          <main ref={setMain} class="min-h-0 flex-1 overflow-y-auto overscroll-contain [scrollbar-gutter:stable_both-edges]">
            <div>
              <Switch>
                <Match when={current().page === "general"}><General /></Match>
                <Match when={current().page === "dictation"}><Dictation /></Match>
                <Match when={current().page === "models"}><Models /></Match>
                <Match when={current().page === "shortcuts"}><Shortcuts /></Match>
                <Match when={current().page === "text"}><TextPage /></Match>
                <Match when={current().page === "snippets"}><Snippets /></Match>
                <Match when={current().page === "notes"}><NotesPage /></Match>
                <Match when={current().page === "words"}><Words /></Match>
                <Match when={current().page === "about"}><About /></Match>
                <Match when={current().page === "kit"}><Kit /></Match>
              </Switch>
            </div>
          </main>
        </div>
      </div>
    </div>
    </Show>
    <Toaster />
    </>
  );
}
