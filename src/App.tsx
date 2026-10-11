/**
 * Purpose: Settings window root: header, sidebar, the scrolling page and the footer slot under it.
 * Contents: App — layout; NAV — page groups (spacing separates groups, as in Raycast's sidebar).
 */
import { Cpu, Info, LayoutGrid, Mic, Settings, Workflow } from "lucide-solid";
import { Match, Switch } from "solid-js";
import { About } from "./pages/About";
import { General } from "./pages/General";
import { Kit } from "./pages/Kit";
import { Models } from "./pages/Models";
import { Rules } from "./pages/Rules";
import { Dictation } from "./pages/Dictation";
import { current, detailTitle, go } from "./nav";
import { Sidebar, type NavDef } from "./ui/Sidebar";
import { TitleBar } from "./ui/TitleBar";
import { setFooterSlot } from "./ui/footer";

const GRAY = "#71717a";
const NAV: NavDef[][] = [
  [{ id: "general", label: "General", icon: Settings, color: GRAY, hero: false }],
  [
    { id: "dictation", label: "Dictation", icon: Mic, color: "#3b82f6", hero: true },
    { id: "models", label: "Models", icon: Cpu, color: "#14b8a6", hero: true },
    { id: "rules", label: "Rules", icon: Workflow, color: "#8b5cf6", hero: true },
  ],
  [
    { id: "about", label: "About", icon: Info, color: GRAY, hero: false },
    ...(import.meta.env.DEV ? [{ id: "kit", label: "Kit", icon: LayoutGrid, color: GRAY, hero: false }] : []),
  ],
];

/** Hero pages name themselves; the others (and any sub-view) put their name in the header. */
function headerTitle() {
  const item = NAV.flat().find((n) => n.id === current().page);
  if (current().detail) return detailTitle();
  return item && !item.hero ? item.label : "";
}

export default function App() {
  return (
    <div class="flex h-full flex-col bg-bg">
      <TitleBar title="Cue" page={headerTitle()} />
      <div class="flex min-h-0 flex-1">
        <Sidebar groups={NAV} active={current().page} onSelect={go} />
        <div class="flex min-h-0 min-w-0 flex-1 flex-col">
          <main class="min-h-0 flex-1 overflow-y-auto [scrollbar-gutter:stable_both-edges]">
            <Switch>
              <Match when={current().page === "general"}><General /></Match>
              <Match when={current().page === "dictation"}><Dictation /></Match>
              <Match when={current().page === "models"}><Models /></Match>
              <Match when={current().page === "rules"}><Rules /></Match>
              <Match when={current().page === "about"}><About /></Match>
              <Match when={current().page === "kit"}><Kit /></Match>
            </Switch>
          </main>
          <div ref={setFooterSlot} class="shrink-0 empty:hidden" />
        </div>
      </div>
    </div>
  );
}
