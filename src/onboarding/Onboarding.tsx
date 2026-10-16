/**
 * Purpose: The first-run walkthrough: a welcome, the features, then one setup page per feature (its shortcut, and a download where one is needed), each a centered column, with a row of step markers, Back and Continue at the foot.
 * It remembers the page the user reached (`onboarding_step`), so closing the window or quitting midway resumes there; Done ends it for good (`onboarded`) and nothing is said afterwards, whatever was or was not downloaded.
 * Contents: Onboarding — the window body; Steps — the markers, with the accent one sliding to the current step. The page state is in `useWalkthrough`.
 */
import { ChevronLeft } from "lucide-solid";
import { For, Match, onMount, Switch } from "solid-js";
import { useModels } from "../useModels";
import { loadShortcuts } from "../ui/useShortcutRecorder";
import { Button } from "../ui/kit";
import { WindowControls } from "../ui/TitleBar";
import { Features, Setup, Welcome } from "./steps";
import { PAGES, STEP_COUNT } from "./pages";
import { useWalkthrough } from "./useWalkthrough";

/** The markers, one per step; the accent one slides to the current step (transform only, 300 ms). It moves by its own width plus the gap, so changing the sizes needs no other edit. */
function Steps(p: { at: number }) {
  return (
    <div class="justify-self-center" role="progressbar" aria-label="Step" aria-valuemin={1} aria-valuemax={STEP_COUNT} aria-valuenow={p.at + 1}>
      <div class="relative flex gap-1.5">
        <For each={Array.from({ length: STEP_COUNT })}>{() => <span class="h-1 w-6 rounded-full bg-selected" />}</For>
        <span class="ease-snap absolute left-0 top-0 h-1 w-6 rounded-full bg-accent transition-transform duration-300" style={{ transform: `translateX(calc(${p.at} * (100% + 0.375rem)))` }} />
      </div>
    </div>
  );
}

export function Onboarding(p: { onDone: () => void }) {
  const models = useModels();
  const { at, go, next, label } = useWalkthrough(() => p.onDone());
  onMount(() => loadShortcuts(PAGES.map((page) => page.shortcut)));
  return (
    <div class="flex h-full flex-col bg-bg">
      <header data-tauri-drag-region class="flex h-10 shrink-0 items-center justify-between">
        <span data-tauri-drag-region class="t-label px-4 text-muted">Cue</span>
        <WindowControls />
      </header>
      <main class="thin-scroll min-h-0 flex-1 overflow-y-auto">
        <Switch>
          <Match when={at() === 0}><Welcome /></Match>
          <Match when={at() === 1}><Features /></Match>
          {/* keyed: each page is built again, so its shortcut control is made for that page's shortcut */}
          <Match when={PAGES[at() - 2]} keyed>{(page) => <Setup {...page} models={models} />}</Match>
        </Switch>
      </main>
      <footer class="grid h-12 shrink-0 grid-cols-3 items-center px-4">
        <button onClick={() => go(at() - 1)} aria-label="Back" class="grid size-7 place-items-center justify-self-start text-muted" classList={{ invisible: at() === 0 }}><ChevronLeft size={16} stroke-width={1.5} /></button>
        <Steps at={at()} />
        <div class="flex justify-self-end"><Button onClick={next}>{label()}</Button></div>
      </footer>
    </div>
  );
}
