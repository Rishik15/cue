/**
 * File: App.tsx
 * Purpose: Settings window root: header, sidebar and the scrolling page.
 * Contents: App — layout (also follows Rust's request to open a page, through `goTo`); headerTitle / feature — the header's page name and feature switch, read from the page catalog; useScrollPerView — a page opens at its top, a sub-view too, and leaving a sub-view puts the list back where it was.
 */
import { createComputed, createEffect, createSignal, on, onCleanup, onMount, Show } from "solid-js";
import { Dynamic } from "solid-js/web";
import { smoothScroll } from "./smoothScroll";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { current, detailTitle, go } from "./nav";
import { PAGES, pageById } from "./pages/catalog";
import { goTo } from "./route";
import { Sidebar } from "./ui/Sidebar";
import { Onboarding } from "./onboarding/Onboarding";
import { useSetting } from "./settings";
import { Toaster } from "./ui/Toast";
import { watchUpdates } from "./useUpdate";
import { TitleBar } from "./ui/TitleBar";

/** The page name, or the name of the sub-view (the rule editor) when one is open. */
const headerTitle = () => (current().detail ? detailTitle() : pageById(current().page).label);

/** The feature the open page is (its on/off setting in the header); the toggle belongs on the page itself, not its sub-views (the rule or snippet editor). */
const feature = () => (current().detail ? undefined : pageById(current().page).feature);

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

/** The settings window. Return: the onboarding until it is done, then the header, sidebar and the open page. */
export default function App() {
  const [main, setMain] = createSignal<HTMLElement>();
  const [onboarded, setOnboarded] = useSetting("onboarded", false);
  useScrollPerView(main);
  // Lenis needs the content element to exist, so it starts after the first render.
  onMount(() => { const page = main(); if (page) onCleanup(smoothScroll(page)); });
  onMount(() => onCleanup(watchUpdates()));
  // Rust sends the window to a page (About, when an update waits): Rust keeps the page until the window takes it: asked for once the listener exists, and again on each `cue-goto`.
  onMount(() => {
    const takePage = () => void invoke<string | null>("take_start_page").then((page) => page && goTo(page), () => undefined);
    const off = listen("cue-goto", takePage);
    void off.then(takePage, () => undefined);
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  return (
    <>
    <Show when={onboarded()} fallback={<Onboarding onDone={() => setOnboarded(true)} />}>
    <div class="flex h-full flex-col bg-bg">
      <TitleBar title="Cue" page={headerTitle()} feature={feature()?.key} featureRow={feature()?.row} />
      <div class="flex min-h-0 flex-1">
        <Sidebar items={PAGES} active={current().page} onSelect={go} />
        <div class="flex min-h-0 min-w-0 flex-1 flex-col">
          <main ref={setMain} class="min-h-0 flex-1 overflow-y-auto overscroll-contain [scrollbar-gutter:stable_both-edges]">
            <div>
              <Dynamic component={pageById(current().page).component} />
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
