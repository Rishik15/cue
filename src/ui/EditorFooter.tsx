/**
 * Purpose: The row under a snippet or folder form: Delete (two clicks) at the left for something that exists, Cancel and Save at the right. Cancel leaves the form without saving.
 * Contents: EditorFooter.
 */
import { Show } from "solid-js";
import { closeDetail } from "../nav";
import { ConfirmButton } from "./ConfirmButton";
import { Button } from "./kit";

export function EditorFooter(p: { what: string; onSave: () => void; onDelete?: () => void }) {
  return (
    <div class="flex items-center justify-between">
      <Show when={p.onDelete} fallback={<span />}>
        <ConfirmButton onConfirm={() => p.onDelete?.()} idle={(arm) => <Button danger onClick={arm}>Delete {p.what}</Button>} />
      </Show>
      <div class="flex gap-2">
        <Button onClick={closeDetail}>Cancel</Button>
        <Button onClick={p.onSave}>Save {p.what}</Button>
      </div>
    </div>
  );
}
