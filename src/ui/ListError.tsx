/**
 * Purpose: The notice shown above a model list that could not be read, with the Retry that asks for it again.
 * Contents: ListError — hidden while the list is readable.
 */
import { Show } from "solid-js";
import { Button } from "./kit";

/** Banner for an unreadable model list. Input: whether the list is unreadable and the retry handler. Renders nothing when it is readable. */
export function ListError(p: { unreadable: boolean; onRetry: () => void }) {
  return (
    <Show when={p.unreadable}>
      <div class="t-hint flex items-center gap-3 px-3" role="alert">
        Could not read the model list.
        <Button onClick={p.onRetry}>Retry</Button>
      </div>
    </Show>
  );
}
