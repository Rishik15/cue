/**
 * Purpose: Settings window root: header, sidebar and the scrolling page.
 * Contents: App — layout; NAV — page list; an item with `section` starts a new sidebar group (app settings, then one page per feature, then About).
 */
import { LayoutGrid } from "lucide-solid";
import { createSignal, Match, onCleanup, onMount, Switch } from "solid-js";
import { smoothScroll } from "./smoothScroll";
import { About } from "./pages/About";
import { Capture } from "./pages/Capture";
import { Dictation } from "./pages/Dictation";
import { General } from "./pages/General";
import { Kit } from "./pages/Kit";
import { Models } from "./pages/Models";
import { Shortcuts } from "./pages/Shortcuts";
import { Snippets } from "./pages/Snippets";
import { Text as TextPage } from "./pages/Text";
import { current, detailTitle, go } from "./nav";
import { Ball, Chip, Gear, Keys, Mic, Scan, Snippet, Text } from "./ui/glyphs";
import { Sidebar, type NavDef } from "./ui/Sidebar";
import { TitleBar } from "./ui/TitleBar";

const NAV: NavDef[] = [
  { id: "general", label: "General", icon: Gear },
  { id: "shortcuts", label: "Shortcuts", icon: Keys },
  { id: "models", label: "Models", icon: Chip },
  { id: "dictation", label: "Dictation", icon: Mic, section: true },
  { id: "text", label: "Text", icon: Text },
  { id: "capture", label: "Capture", icon: Scan },
  { id: "snippets", label: "Snippets", icon: Snippet },
  { id: "about", label: "About", icon: Ball, section: true },
  ...(import.meta.env.DEV ? [{ id: "kit", label: "Kit", icon: LayoutGrid }] : []),
];

/** The page name, or the name of the sub-view (the rule editor) when one is open. */
const headerTitle = () => (current().detail ? detailTitle() : (NAV.find((n) => n.id === current().page)?.label ?? ""));

export default function App() {
  const [main, setMain] = createSignal<HTMLElement>();
  // Lenis needs the content element to exist, so it starts after the first render.
  onMount(() => { const page = main(); if (page) onCleanup(smoothScroll(page)); });
  return (
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
                <Match when={current().page === "about"}><About /></Match>
                <Match when={current().page === "kit"}><Kit /></Match>
              </Switch>
            </div>
          </main>
        </div>
      </div>
    </div>
  );
}
