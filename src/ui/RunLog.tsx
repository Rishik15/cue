/**
 * Purpose: The recent rule runs: when, which rules matched and in which program. No text is shown or kept.
 * Contents: RunLog — a section with the last runs, read when the page opens; ago — a short relative time.
 */
import { createSignal, For, onMount, Show } from "solid-js";
import { getLog, labelOf, TRIGGERS, type LogEntry } from "../rules";
import { Card, ListRow, Section } from "./kit";

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
            <ListRow title={r.matched.join(", ")} subtitle={[labelOf(TRIGGERS, r.trigger), r.app].filter(Boolean).join(" · ")}>
              <span class="t-hint">{ago(r.at)}</span>
            </ListRow>
          )}
        </For>
        <Show when={runs().length === 0}><p class="t-hint px-3 py-[13px]">No rule has run since Cue started.</p></Show>
      </Card>
    </Section>
  );
}
