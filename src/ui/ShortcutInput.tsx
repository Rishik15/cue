/**
 * File: ShortcutInput.tsx
 * Purpose: Shortcut row control for one catalog entry: current combo chip, reset button, on/off button, and a translucent recording popover above the row.
 * Contents: ShortcutInput — chip opens the popover; RecordingPopup — live keycaps, hint, a gradient sweep on each save, footer.
 * Recording state lives in useShortcutRecorder.
 */
import * as Popover from "@kobalte/core/popover";
import { Ban, RotateCcw } from "lucide-solid";
import { Show, untrack } from "solid-js";
import { KeyCaps, KeyChip, type Part } from "./KeyChip";
import { useSetting } from "../settings";
import { IconButton, Kbd } from "./kit";
import { useShortcutRecorder } from "./useShortcutRecorder";

/** The recording popup: live keycaps, the hint, and a gradient that glows up from the bottom each time `saves` grows. Problems are not shown here but in the window's toast. The prompt to press keys shows only once the recording is armed (`ready`); before that a key would pass to the app unrecorded. Input: the keys held now, how many saves this recording has made and whether it is armed. */
function RecordingPopup(props: { live: Part[]; saves: number; ready: boolean }) {
  return (
    <Popover.Content data-shortcut-recording class="popup relative z-50 flex h-[104px] w-[240px] flex-col overflow-hidden rounded-lg border border-line bg-popup shadow-popup backdrop-blur-xl">
      <Show when={props.saves} keyed><span class="save-sheen" /></Show>
      <div class="flex flex-1 flex-col items-center justify-center gap-2">
        <div class="flex h-7 items-center"><Show when={props.live.length}><KeyCaps parts={props.live} /></Show></div>
        <span class="t-micro">{props.ready ? "Press the keys you want to use" : "Getting ready…"}</span>
      </div>
      <div class="t-micro flex h-7 items-center justify-between border-t border-line px-3">
        <span>Cue</span>
        <span class="flex items-center gap-1.5">Cancel <Kbd>Esc</Kbd></span>
      </div>
    </Popover.Content>
  );
}

/** The control for one shortcut: its chip (click to record), reset, and its on/off button. Input: the shortcut's catalog `id`. */
export function ShortcutInput(props: { id: string }) {
  const id = untrack(() => props.id); // the id never changes while the control exists
  const r = useShortcutRecorder(id);
  const label = () => r.row()?.label ?? id;
  const [on, setOn] = useSetting(`${id}_shortcut_on`, true); // the shortcut's own on/off; off keeps the combo but greys it
  return (
    // No onOpenChange: the popup must not close itself on a focus move or pointer event (a window losing focus for a moment ended recordings); the recorder cancels on a click outside, or Escape in Rust.
    <Popover.Root open={r.open()} placement="top-end" gutter={10} modal={false}>
      <Popover.Anchor class="flex items-center gap-2" data-shortcut-recording>
        <button onClick={() => void r.begin()} aria-label={`Change ${label()} shortcut`} class="rounded-md" classList={{ "opacity-40": !on() }}>
          <KeyChip parts={r.parts()} held={r.held()} />
        </button>
        <IconButton label={`Reset ${label()} shortcut`} onClick={r.reset}><RotateCcw size={14} /></IconButton>
        <IconButton label={`${on() ? "Turn off" : "Turn on"} ${label()} shortcut`} onClick={() => setOn(!on())}>
          <Ban size={14} classList={{ "text-accent": !on() }} />
        </IconButton>
      </Popover.Anchor>
      <Popover.Portal><RecordingPopup live={r.live()} saves={r.saves()} ready={r.ready()} /></Popover.Portal>
    </Popover.Root>
  );
}
