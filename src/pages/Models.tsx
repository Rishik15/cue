/**
 * Purpose: Models settings page: download, choose and delete the speech model, the text model that Correct and Rewrite use.
 * Contents: Models — page assembled from the UI kit.
 */
import { For, Show } from "solid-js";
import { useSetting } from "../settings";
import { useModels } from "../useModels";
import { ModelCard } from "../ui/ModelCard";
import { Card, Page, Section } from "../ui/kit";

function TextModels(p: { models: ReturnType<typeof useModels> }) {
  const text = () => p.models.models().filter((x) => x.kind === "text");
  const [selected, select] = useSetting("text_model", "qwen2.5-1.5b");
  // Matches the text engine: the chosen model if it is installed, otherwise the first installed one.
  const inUse = () => {
    const installed = text().filter((x) => x.installed);
    return (installed.find((x) => x.id === selected()) ?? installed[0])?.id;
  };
  return (
    <Show when={text().length > 0}>
      <Section id="text-models" title="Text Model">
        <Card>
          <For each={text()}>
            {(model) => (
              <ModelCard
                model={model}
                selected={inUse() === model.id}
                progress={p.models.progress()[model.id]}
                blocked={p.models.busy()}
                onUse={() => select(model.id)}
                onDownload={() => p.models.download(model.id)}
                onCancel={() => p.models.cancel(model.id)}
                onDelete={() => p.models.remove(model.id)}
              />
            )}
          </For>
        </Card>
      </Section>
    </Show>
  );
}

export function Models() {
  const m = useModels();
  const speech = () => m.models().filter((x) => x.kind === "speech");
  const [selected, select] = useSetting("speech_model", "parakeet-v2");
  // Matches what the speech engine does: the chosen model if it is installed, otherwise the first installed one.
  const inUse = () => {
    const installed = speech().filter((x) => x.installed);
    return (installed.find((x) => x.id === selected()) ?? installed[0])?.id;
  };
  return (
    <Page>
      <Section id="speech-models" title="Speech Models">
        <Card>
          <For each={speech()} fallback={<p class="t-hint p-3">No models found.</p>}>
            {(model) => (
              <ModelCard
                model={model}
                selected={inUse() === model.id}
                progress={m.progress()[model.id]}
                blocked={m.busy()}
                onUse={() => select(model.id)}
                onDownload={() => m.download(model.id)}
                onCancel={() => m.cancel(model.id)}
                onDelete={() => m.remove(model.id)}
              />
            )}
          </For>
        </Card>
        <p class="t-hint px-3">Short phrase models cut long recordings at pauses; long session models stream without cuts.</p>
      </Section>
      <TextModels models={m} />
    </Page>
  );
}
