/**
 * File: App.tsx
 * Purpose: Settings window root: header, sidebar and the scrolling page.
 * Contents: App — layout; headerTitle / feature — the header's page name and feature switch, read from the page catalog.
 */
import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { Dynamic } from "solid-js/web";
import { smoothScroll } from "./smoothScroll";
import { current, detailTitle, go } from "./nav";
import { PAGES, pageById } from "./pages/catalog";
import { useStartPage } from "./route";
import { Sidebar } from "./ui/Sidebar";
import { Onboarding } from "./onboarding/Onboarding";
import { useSetting } from "./settings";
import { Toaster } from "./ui/Toast";
import { watchUpdates } from "./useUpdate";
import { TitleBar } from "./ui/TitleBar";
import { useScrollPerView } from "./useScrollPerView";

/** The page name, or the name of the sub-view (the rule editor) when one is open. */
const headerTitle = () => (current().detail ? detailTitle() : pageById(current().page).label);

/** The feature the open page is (its on/off setting in the header); the toggle belongs on the page itself, not its sub-views (the rule or snippet editor). */
const feature = () => (current().detail ? undefined : pageById(current().page).feature);

/** The settings window. Return: the onboarding until it is done, then the header, sidebar and the open page. */
export default function App() {
  const [main, setMain] = createSignal<HTMLElement>();
  const [onboarded, setOnboarded] = useSetting("onboarded", false);
  useScrollPerView(main);
  // Lenis needs the content element to exist, so it starts after the first render.
  onMount(() => { const page = main(); if (page) onCleanup(smoothScroll(page)); });
  onMount(() => onCleanup(watchUpdates()));
  useStartPage();
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
