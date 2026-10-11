/**
 * Purpose: Settings window root: header, sidebar, the scrolling page and the footer slot under it.
 * Contents: App — layout; NAV — page list.
 */
import { Cpu, Info, LayoutGrid, Mic, Settings, Workflow } from "lucide-solid";
import { Match, onCleanup, Switch } from "solid-js";
import { smoothScroll } from "./smoothScroll";
import { About } from "./pages/About";
import { Dictation } from "./pages/Dictation";
import { General } from "./pages/General";
import { Kit } from "./pages/Kit";
import { Models } from "./pages/Models";
import { Rules } from "./pages/Rules";
import { current, detailTitle, go } from "./nav";
import { Sidebar, type NavDef } from "./ui/Sidebar";
import { TitleBar } from "./ui/TitleBar";
import { setFooterSlot } from "./ui/footer";

const NAV: NavDef[] = [
  { id: "general", label: "General", icon: Settings },
  { id: "dictation", label: "Dictation", icon: Mic },
  { id: "models", label: "Models", icon: Cpu },
  { id: "rules", label: "Rules", icon: Workflow },
  { id: "about", label: "About", icon: Info },
  ...(import.meta.env.DEV ? [{ id: "kit", label: "Kit", icon: LayoutGrid }] : []),
];

/** The page name, or the name of the sub-view (the rule editor) when one is open. */
const headerTitle = () => (current().detail ? detailTitle() : (NAV.find((n) => n.id === current().page)?.label ?? ""));

export default function App() {
  return (
    <div class="flex h-full flex-col bg-bg">
      <TitleBar title="Cue" page={headerTitle()} />
      <div class="flex min-h-0 flex-1">
        <Sidebar items={NAV} active={current().page} onSelect={go} />
        <div class="flex min-h-0 min-w-0 flex-1 flex-col">
          <main ref={(el) => onCleanup(smoothScroll(el))} class="min-h-0 flex-1 overflow-y-auto overscroll-contain [scrollbar-gutter:stable_both-edges]">
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
