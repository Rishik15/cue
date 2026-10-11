/**
 * Purpose: Models settings page: download, choose and delete the speech model.
 * Contents: Models — page assembled from the UI kit.
 */
import { For, Show } from "solid-js";
import { useSetting } from "../settings";
import { useModels } from "../useModels";
import { ModelCard } from "../ui/ModelCard";
import { Card, Page, Section } from "../ui/kit";

export function Models() {
  const m = useModels();
  const [selected, select] = useSetting("speech_model", "parakeet-v2");
  // Matches what the speech engine does: the chosen model if it is installed, otherwise the first installed one.
  const inUse = () => {
    const installed = m.models().filter((x) => x.installed);
    return (installed.find((x) => x.id === selected()) ?? installed[0])?.id;
  };
  return (
    <Page>
      <Section title="Speech">
        <Card>
          <For each={m.models()} fallback={<p class="t-hint p-3">No models found.</p>}>
            {(model) => (
              <ModelCard
                model={model}
                selected={inUse() === model.id}
                progress={m.progress()[model.id]}
                onUse={() => select(model.id)}
                onDownload={() => m.download(model.id)}
                onCancel={() => m.cancel(model.id)}
                onDelete={() => m.remove(model.id)}
              />
            )}
          </For>
        </Card>
        <Show when={m.models().length > 0 && !inUse()}>
          <p class="t-hint px-3">Download a model to start dictating.</p>
        </Show>
        <Show when={m.error()}><p class="t-hint px-3 !text-danger">{m.error()}</p></Show>
      </Section>
    </Page>
  );
}
