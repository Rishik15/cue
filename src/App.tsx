/**
 * Purpose: Settings window root: sidebar navigation and page switching.
 * Contents: App — layout; NAV — page list.
 */
import { Cpu, Info, LayoutGrid, Mic, Settings, Type, Workflow } from "lucide-solid";
import { createSignal, Match, Switch } from "solid-js";
import { About } from "./pages/About";
import { General } from "./pages/General";
import { Kit } from "./pages/Kit";
import { Models } from "./pages/Models";
import { Voice } from "./pages/Voice";
import { Sidebar, type NavDef } from "./ui/Sidebar";
import { TitleBar } from "./ui/TitleBar";
import { Page } from "./ui/kit";

const NAV: NavDef[] = [
  { id: "general", label: "General", icon: Settings },
  { id: "voice", label: "Voice", icon: Mic },
  { id: "text", label: "Text", icon: Type },
  { id: "models", label: "Models", icon: Cpu },
  { id: "rules", label: "Rules", icon: Workflow },
  { id: "about", label: "About", icon: Info },
  ...(import.meta.env.DEV ? [{ id: "kit", label: "Kit", icon: LayoutGrid }] : []),
];

export default function App() {
  const [page, setPage] = createSignal("general");
  return (
    <div class="flex h-full flex-col bg-bg">
      <TitleBar title="Cue" page={NAV.find((n) => n.id === page())!.label} />
      <div class="flex min-h-0 flex-1">
        <Sidebar items={NAV} active={page()} onSelect={setPage} />
        <main class="min-h-0 min-w-0 flex-1 overflow-y-auto [scrollbar-gutter:stable_both-edges]">
          <Switch fallback={<Page><p class="t-hint px-3">Coming in a later step.</p></Page>}>
            <Match when={page() === "general"}><General /></Match>
            <Match when={page() === "voice"}><Voice /></Match>
            <Match when={page() === "models"}><Models /></Match>
            <Match when={page() === "about"}><About /></Match>
            <Match when={page() === "kit"}><Kit /></Match>
          </Switch>
        </main>
      </div>
    </div>
  );
}
