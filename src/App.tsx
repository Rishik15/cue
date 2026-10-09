/**
 * Purpose: Settings window root: sidebar navigation and page switching.
 * Contents: App — layout; NAV — page list; General — General settings page built from the UI kit.
 */
import { Cpu, Info, Mic, Settings, Type, Workflow } from "lucide-solid";
import { invoke } from "@tauri-apps/api/core";
import { createSignal, Match, onMount, Switch } from "solid-js";
import { useSetting } from "./settings";
import { family, mode as appearance, MODES, setFamily, setMode as setAppearance, type Mode } from "./theme";
import { Sidebar, type NavDef } from "./ui/Sidebar";
import { TitleBar } from "./ui/TitleBar";
import { Card, Page, Row, Section, Segmented, Select, Toggle } from "./ui/kit";
import { ThemePicker } from "./ui/ThemePicker";
import { ShortcutInput } from "./ui/ShortcutInput";

const NAV: NavDef[] = [
  { id: "general", label: "General", icon: Settings },
  { id: "voice", label: "Voice", icon: Mic },
  { id: "text", label: "Text", icon: Type },
  { id: "models", label: "Models", icon: Cpu },
  { id: "rules", label: "Rules", icon: Workflow },
  { id: "about", label: "About", icon: Info },
];

function General() {
  const [login, setLogin] = createSignal(false);
  onMount(() => invoke<boolean>("get_autostart").then(setLogin));
  const toggleLogin = (on: boolean) => {
    setLogin(on);
    invoke("set_autostart", { on }).catch(() => setLogin(!on));
  };
  const [mode, setMode] = useSetting("dictation_mode", "Hold");
  const [unload, setUnload] = useSetting("unload_after", "After 2 Minutes");
  return (
    <Page>
      <Section>
        <Card>
          <Row label="Launch at Login" hint="Start Cue quietly in the tray when you sign in."><Toggle label="Launch at Login" checked={login()} onChange={toggleLogin} /></Row>
          <Row label="Dictation Shortcut" hint="The keys that start dictation."><ShortcutInput /></Row>
          <Row label="Dictation Mode" hint="Hold the shortcut while speaking, or press once to start and again to stop."><Segmented label="Dictation Mode" value={mode()} onChange={setMode} options={["Hold", "Toggle"]} /></Row>
        </Card>
      </Section>
      <Section title="Appearance">
        <Card>
          <Row label="Mode" hint="Follow Windows, or always use light or dark."><Segmented label="Mode" value={appearance()} onChange={(v) => setAppearance(v as Mode)} options={MODES} /></Row>
          <ThemePicker label="Theme" hint="Every theme comes in a dark and a light version." value={family()} onChange={setFamily} />
        </Card>
      </Section>
      <Section title="Performance">
        <Card>
          <Row label="Unload Models" hint="Free memory after Cue has been idle."><Select label="Unload Models" value={unload()} onChange={setUnload} options={["Never", "After 2 Minutes", "After 5 Minutes", "Immediately"]} /></Row>
        </Card>
      </Section>
    </Page>
  );
}

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
          </Switch>
        </main>
      </div>
    </div>
  );
}
