/**
 * Purpose: Models settings page: download, choose and delete the speech model, and download or delete the text model that Correct and Rewrite use.
 * Contents: Models — page assembled from the UI kit.
 */
import { For, Show } from "solid-js";
import { useSetting } from "../settings";
import { useModels } from "../useModels";
import { ModelCard } from "../ui/ModelCard";
import { Card, Page, Section } from "../ui/kit";

function TextModels(p: { models: ReturnType<typeof useModels> }) {
  const text = () => p.models.models().filter((x) => x.kind === "text");
  return (
    <Show when={text().length > 0}>
      <Section id="text-models" title="Text">
        <Card>
          <For each={text()}>
            {(model) => (
              <ModelCard
                model={model}
                selected={false}
                progress={p.models.progress()[model.id]}
                onDownload={() => p.models.download(model.id)}
                onCancel={() => p.models.cancel(model.id)}
                onDelete={() => p.models.remove(model.id)}
              />
            )}
          </For>
        </Card>
        <p class="t-hint px-3">Used by the Correct Text and Make actions in Rules. It loads only while a rule runs.</p>
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
      <Section id="speech-models" title="Speech">
        <Card>
          <For each={speech()} fallback={<p class="t-hint p-3">No models found.</p>}>
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
        <Show when={speech().length > 0 && !inUse()}>
          <p class="t-hint px-3">Download a model to start dictating.</p>
        </Show>
        <Show when={m.error()}><p class="t-hint px-3 !text-danger">{m.error()}</p></Show>
      </Section>
      <TextModels models={m} />
    </Page>
  );
}
