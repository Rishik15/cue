/**
 * Purpose: The recent rule runs: when, which rules matched and in which program. No text is shown or kept.
 * Contents: RunLog — a section with the last runs, read when the page opens; ago — a short relative time.
 */
import { createSignal, For, onMount, Show } from "solid-js";
import { getLog, labelOf, TRIGGERS, type LogEntry } from "../rules";
import { Card, Section } from "./kit";

const ago = (at: number) => {
  const s = Math.max(0, Math.floor(Date.now() / 1000) - at);
  if (s < 60) return "just now";
  return s < 3600 ? `${Math.floor(s / 60)} min ago` : `${Math.floor(s / 3600)} h ago`;
};

export function RunLog() {
  const [runs, setRuns] = createSignal<LogEntry[]>([]);
  onMount(() => getLog().then(setRuns, () => setRuns([])));
  return (
    <Section title="Recent Runs">
      <Card>
        <For each={runs().slice(0, 8)}>
          {(r) => (
            <div class="flex items-center justify-between gap-6 border-t border-line px-3 py-[13px] first:border-t-0">
              <div class="flex min-w-0 flex-col gap-0.5">
                <span class="t-label truncate" title={r.matched.join(", ")}>{r.matched.join(", ")}</span>
                <span class="t-hint truncate">{labelOf(TRIGGERS, r.trigger)}<Show when={r.app}> · {r.app}</Show></span>
              </div>
              <span class="t-hint shrink-0">{ago(r.at)}</span>
            </div>
          )}
        </For>
        <Show when={runs().length === 0}><p class="t-hint px-3 py-[13px]">No rule has run since Cue started.</p></Show>
      </Card>
    </Section>
  );
}
