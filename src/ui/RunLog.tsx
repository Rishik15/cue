/**
 * File: ui/RunLog.tsx
 * Purpose: The recent rule runs: when, which rules matched and in which program. No text is shown or kept. A log that could not be read is shown as an error, never as "no runs".
 * Contents: RunLog — a section with the last runs, read when the page opens; ago — a short relative time.
 */
import { createSignal, For, onMount, Show } from "solid-js";
import { reason } from "../ipc";
import { getLog, type LogEntry, type Trigger } from "../rules";
import { Card, Hint, ListRow, Section } from "./kit";

/** How long ago a run was, in short words. Input: seconds since 1970. Return: the text. */
const ago = (at: number) => {
  const s = Math.max(0, Math.floor(Date.now() / 1000) - at);
  if (s < 60) return "just now";
  return s < 3600 ? `${Math.floor(s / 60)} min ago` : `${Math.floor(s / 3600)} h ago`;
};

/** The last runs of one trigger's rules. Input: the trigger. Return: the section. */
export function RunLog(p: { trigger: Trigger }) {
  const [runs, setRuns] = createSignal<LogEntry[]>([]);
  const [error, setError] = createSignal("");
  onMount(() => getLog().then((l) => setRuns(l.filter((r) => r.trigger === p.trigger)), (e) => setError(reason(e))));
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
        <Show when={runs().length === 0}>
          <Show when={error()} fallback={<Hint>No runs since Cue started</Hint>}>
            <Hint danger>{error()}</Hint>
          </Show>
        </Show>
      </Card>
    </Section>
  );
}
