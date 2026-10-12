/**
 * Purpose: The row under Dictation Mode that says which model long sessions use and takes you to Models to change or download it.
 * Contents: LiveModelRow — the model in use as a value with a chevron, or a Download button when no Live Dictation model is installed.
 */
import { ChevronRight } from "lucide-solid";
import { Show } from "solid-js";
import { go } from "../nav";
import { useSetting } from "../settings";
import { useModels } from "../useModels";
import { Button, Row } from "./kit";

export function LiveModelRow() {
  const models = useModels();
  const [chosen] = useSetting("stream_model", "");
  // Matches the engine: the chosen model if it is installed, otherwise the first installed one.
  const inUse = () => {
    const installed = models.models().filter((m) => m.kind === "stream" && m.installed);
    return installed.find((m) => m.id === chosen()) ?? installed[0];
  };
  return (
    <Row id="live-model" label="Live Dictation Model" hint="Types as you speak in long sessions.">
      <Show when={models.models().length > 0}>
        <Show when={inUse()} fallback={<Button onClick={() => go("models")}>Download</Button>}>
          {(m) => (
            <button onClick={() => go("models")} class="t-control flex h-7 max-w-[16rem] items-center gap-1.5 rounded-md pl-2 pr-1" aria-label="Change the Live Dictation model">
              <span class="truncate" title={m().name}>{m().name}</span>
              <ChevronRight size={14} class="shrink-0 text-muted" />
            </button>
          )}
        </Show>
      </Show>
    </Row>
  );
}
