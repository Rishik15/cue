/**
 * Purpose: The recent rule runs: when, which rules matched and in which program. No text is shown or kept.
 * Contents: RunLog — a section with the last runs, read when the page opens; ago — a short relative time.
 */
import { createSignal, For, onMount, Show } from "solid-js";
import { getLog, type LogEntry, type Trigger } from "../rules";
import { Card, ListRow, Section } from "./kit";

const ago = (at: number) => {
  const s = Math.max(0, Math.floor(Date.now() / 1000) - at);
  if (s < 60) return "just now";
  return s < 3600 ? `${Math.floor(s / 60)} min ago` : `${Math.floor(s / 3600)} h ago`;
};

export function RunLog(p: { trigger: Trigger }) {
  const [runs, setRuns] = createSignal<LogEntry[]>([]);
  onMount(() => getLog().then((l) => setRuns(l.filter((r) => r.trigger === p.trigger)), () => setRuns([])));
  return (
    <Section title="Recent Runs">
      <Card>
        <For each={runs().slice(0, 8)}>
          {(r) => (
            <ListRow title={r.matched.join(", ")} subtitle={r.app}>
              <span class="t-hint">{ago(r.at)}</span>
            </ListRow>
          )}
        </For>
        <Show when={runs().length === 0}><p class="t-hint px-3 py-[13px]">No action has run since Cue started.</p></Show>
      </Card>
    </Section>
  );
}
