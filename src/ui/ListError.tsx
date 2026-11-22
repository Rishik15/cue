/**
 * File: ui/ListError.tsx
 * Purpose: The notice shown above a list that could not be read (models by default, or what `message` names), with the Retry that asks for it again.
 * Contents: ListError — hidden while the list is readable.
 */
import { Show } from "solid-js";
import { Button } from "./kit";

/** Banner for an unreadable list. Input: whether it is unreadable, the retry handler and, optionally, the sentence to show (default: the model list). Renders nothing when it is readable. */
export function ListError(p: { unreadable: boolean; onRetry: () => void; message?: string }) {
  return (
    <Show when={p.unreadable}>
      <div class="t-hint flex items-center gap-3 px-3" role="alert">
        {p.message ?? "Could not read the model list."}
        <Button onClick={p.onRetry}>Retry</Button>
      </div>
    </Show>
  );
}
