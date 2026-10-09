/**
 * Purpose: Shortcut row control: current combo chip, reset button, and a translucent recording popover above the row.
 * Contents: ShortcutInput — chip opens the popover; the Rust engine streams live keys (`shortcut-live`),
 * finishes on release (`shortcut-done`), and reports unusable combos (`shortcut-invalid`).
 */
import * as Popover from "@kobalte/core/popover";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { RotateCcw } from "lucide-solid";
import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { KeyCaps, KeyChip, type Part } from "./KeyChip";

type Done = { status: "ok" | "cancelled"; parts: Part[] };
const swallow = (e: Event) => { e.preventDefault(); e.stopPropagation(); };

export function ShortcutInput() {
  const [parts, setParts] = createSignal<Part[]>([]);
  const [open, setOpen] = createSignal(false);
  const [live, setLive] = createSignal<Part[]>([]);
  const [error, setError] = createSignal("");
  const [held, setHeld] = createSignal(false);
  const unlisten: Promise<UnlistenFn>[] = [];

  onMount(() => {
    invoke<Part[]>("get_shortcut").then(setParts);
    unlisten.push(
      listen<boolean>("dictation-shortcut", (e) => setHeld(e.payload)),
      listen<Part[]>("shortcut-live", (e) => { setError(""); setLive(e.payload); }),
      listen<string>("shortcut-invalid", (e) => { setLive([]); setError(e.payload); }),
      listen<Done>("shortcut-done", (e) => { setParts(e.payload.parts); close(); }),
    );
  });
  onCleanup(() => unlisten.forEach((u) => u.then((f) => f())));

  const keys = ["keydown", "keyup", "keypress"] as const;
  function begin() {
    setLive([]);
    setError("");
    setOpen(true);
    invoke("shortcut_record", { start: true });
    // The webview also receives the keys; stop them from activating controls under the popover.
    keys.forEach((k) => window.addEventListener(k, swallow, true));
  }
  function close() {
    keys.forEach((k) => window.removeEventListener(k, swallow, true));
    setOpen(false);
  }
  const cancel = () => { invoke("shortcut_record", { start: false }); close(); };

  return (
    <Popover.Root open={open()} onOpenChange={(o) => !o && cancel()} placement="top-end" gutter={10} modal={false}>
      <Popover.Anchor class="flex items-center gap-2">
        <button onClick={begin} aria-label="Change shortcut" class="rounded-md"><KeyChip parts={parts()} held={held()} /></button>
        <button
          onClick={() => invoke<Part[]>("reset_shortcut").then(setParts)}
          aria-label="Reset shortcut"
          class="grid size-7 place-items-center rounded-md bg-control text-muted"
        >
          <RotateCcw size={14} />
        </button>
      </Popover.Anchor>
      <Popover.Portal>
        <Popover.Content class="popup z-50 flex h-[104px] w-[240px] flex-col overflow-hidden rounded-lg border border-line bg-card/70 shadow-2xl backdrop-blur-xl">
          <div class="flex flex-1 flex-col items-center justify-center gap-2">
            <div class="flex h-7 items-center"><Show when={live().length}><KeyCaps parts={live()} /></Show></div>
            <Show when={error()} fallback={<span class="t-micro">Press the keys you want to use</span>}>
              <span class="t-micro text-danger">{error()}</span>
            </Show>
          </div>
          <div class="t-micro flex h-7 items-center justify-between border-t border-line px-3">
            <span>Cue</span>
            <span class="flex items-center gap-1.5">Cancel <kbd class="rounded border border-line px-1 font-[inherit]">Esc</kbd></span>
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
