/**
 * Purpose: The small "are you sure?" popup shown before something is deleted (a rule, a downloaded model).
 * Contents: Confirm — wraps the button that starts the deletion; `trigger` receives the function that opens the popup.
 */
import * as Popover from "@kobalte/core/popover";
import { createSignal, type JSX } from "solid-js";
import { Button } from "./kit";

export function Confirm(p: { message: string; confirm: string; onConfirm: () => void; trigger: (open: () => void) => JSX.Element }) {
  const [open, setOpen] = createSignal(false);
  return (
    <Popover.Root open={open()} onOpenChange={setOpen} placement="top-end" gutter={8} modal={false}>
      <Popover.Anchor class="inline-flex">{p.trigger(() => setOpen(true))}</Popover.Anchor>
      <Popover.Portal>
        <Popover.Content class="popup z-50 flex w-60 flex-col gap-3 rounded-lg border border-line bg-popup p-3 shadow-popup backdrop-blur-xl">
          <p class="t-label break-words">{p.message}</p>
          <div class="flex justify-end gap-2">
            <Button onClick={() => setOpen(false)}>Cancel</Button>
            <Button danger onClick={() => { setOpen(false); p.onConfirm(); }}>{p.confirm}</Button>
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
