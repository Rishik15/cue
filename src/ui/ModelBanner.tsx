/**
 * Purpose: The "download a model" banner shown on the pages a new user lands on, until a speech model (offline or streaming) is installed.
 * Contents: ModelBanner — hidden once any model is installed (or while the list is still loading).
 */
import { Show } from "solid-js";
import { go } from "../nav";
import { useModels } from "../useModels";
import { Banner, Button } from "./kit";

export function ModelBanner() {
  const models = useModels();
  const missing = () => models.models().length > 0 && !models.models().some((m) => m.kind !== "text" && m.installed);
  return (
    <Show when={missing()}>
      <Banner title="Download a model to start dictating" hint="The speech model runs on your device. Nothing is sent anywhere.">
        <Button onClick={() => go("models")}>Open Models</Button>
      </Banner>
    </Show>
  );
}
