/**
 * Purpose: Settings window root: header, sidebar and the scrolling page.
 * Contents: App — layout; NAV — page list; an item with `section` starts a new sidebar group (app settings, then one page per feature, then About); useScrollPerView — a page opens at its top, a sub-view too, and leaving a sub-view puts the list back where it was.
 */
import { LayoutGrid } from "lucide-solid";
import { createComputed, createEffect, createSignal, Match, on, onCleanup, onMount, Show, Switch } from "solid-js";
import { smoothScroll } from "./smoothScroll";
import { About } from "./pages/About";
import { Capture } from "./pages/Capture";
import { Dictation } from "./pages/Dictation";
import { General } from "./pages/General";
import { Kit } from "./pages/Kit";
import { Models } from "./pages/Models";
import { Shortcuts } from "./pages/Shortcuts";
import { Snippets } from "./pages/Snippets";
import { Words } from "./pages/Words";
import { Text as TextPage } from "./pages/Text";
import { current, detailTitle, go } from "./nav";
import { Ball, Book, Chip, Gear, Keys, Mic, Scan, Snippet, Text } from "./ui/glyphs";
import { Sidebar, type NavDef } from "./ui/Sidebar";
import { Onboarding } from "./onboarding/Onboarding";
import { useSetting } from "./settings";
import { Toaster } from "./ui/Toast";
import { TitleBar } from "./ui/TitleBar";

const NAV: NavDef[] = [
  { id: "general", label: "General", icon: Gear },
  { id: "shortcuts", label: "Shortcuts", icon: Keys },
  { id: "models", label: "Models", icon: Chip },
  { id: "dictation", label: "Dictation", icon: Mic, section: true },
  { id: "text", label: "Transform Text", icon: Text },
  { id: "capture", label: "Capture", icon: Scan },
  { id: "snippets", label: "Snippets", icon: Snippet },
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
                <Match when={current().page === "capture"}><Capture /></Match>
                <Match when={current().page === "snippets"}><Snippets /></Match>
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
