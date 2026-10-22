/**
 * Purpose: Shortcut row control: current combo chip, reset button, and a translucent recording popover above the row.
 * Contents: ShortcutInput — chip opens the popover; RecordingPopup — live keycaps, hint or error, footer.
 * Recording state lives in useShortcutRecorder.
 */
import * as Popover from "@kobalte/core/popover";
import { RotateCcw } from "lucide-solid";
import { Show, untrack } from "solid-js";
import { KeyCaps, KeyChip, type Part } from "./KeyChip";
import { IconButton, Kbd } from "./kit";
import { useShortcutRecorder } from "./useShortcutRecorder";

function RecordingPopup(props: { live: Part[]; error: string }) {
  return (
    <Popover.Content data-shortcut-recording class="popup z-50 flex h-[104px] w-[240px] flex-col overflow-hidden rounded-lg border border-line bg-popup shadow-popup backdrop-blur-xl">
      <div class="flex flex-1 flex-col items-center justify-center gap-2">
        <div class="flex h-7 items-center"><Show when={props.live.length}><KeyCaps parts={props.live} /></Show></div>
        <Show when={props.error} fallback={<span class="t-micro">Press the keys you want to use</span>}>
          <span class="t-micro text-danger">{props.error}</span>
        </Show>
      </div>
      <div class="t-micro flex h-7 items-center justify-between border-t border-line px-3">
        <span>Cue</span>
        <span class="flex items-center gap-1.5">Cancel <Kbd>Esc</Kbd></span>
      </div>
    </Popover.Content>
  );
}

export function ShortcutInput(props: { name: string }) {
  const r = useShortcutRecorder(untrack(() => props.name)) // the name never changes while the control exists;
  return (
    // No onOpenChange: the popup must not close itself on a focus move or pointer event (a window losing focus for a moment ended recordings); the recorder cancels on a click outside, or Escape in Rust.
    <Popover.Root open={r.open()} placement="top-end" gutter={10} modal={false}>
      <Popover.Anchor class="flex items-center gap-2" data-shortcut-recording>
        <button onClick={r.begin} aria-label={`Change ${props.name} shortcut`} class="rounded-md"><KeyChip parts={r.parts()} held={r.held()} /></button>
        <IconButton label={`Reset ${props.name} shortcut`} onClick={r.reset}><RotateCcw size={14} /></IconButton>
      </Popover.Anchor>
      <Popover.Portal><RecordingPopup live={r.live()} error={r.error()} /></Popover.Portal>
    </Popover.Root>
  );
}
