/**
 * Purpose: Full-width header band split like the body: the app name over the sidebar, over the content the history arrows and
 * the page name; a feature's on/off toggle (when the page is a feature) and the window controls at the right (minimize and close, no maximize).
 * Contents: TitleBar — the header, with a feature's on/off switch when the page is a feature; WindowControls — minimize and close; Control — one caption button; Arrow — one history arrow, greyed out when there is nowhere to go.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ChevronLeft, ChevronRight, Minus, X } from "lucide-solid";
import { Show, type JSX } from "solid-js";
import { back, canBack, canForward, forward } from "../nav";
import { useSetting } from "../settings";
import { Toggle } from "./kit";

function Control(props: { label: string; onClick: () => void; danger?: boolean; children: JSX.Element }) {
  return (
    <button
      aria-label={props.label}
      onClick={() => props.onClick()}
      class="grid h-10 w-[45px] place-items-center text-muted"
      classList={{ "hover:bg-close hover:text-white": props.danger, "hover:bg-hover hover:text-ink": !props.danger }}
    >
      {props.children}
    </button>
  );
}

function Arrow(props: { label: string; enabled: boolean; onClick: () => void; children: JSX.Element }) {
  return (
    <button
      aria-label={props.label}
      disabled={!props.enabled}
      onClick={() => props.onClick()}
      class="grid h-7 w-5 place-items-center rounded-md text-ink disabled:text-muted disabled:opacity-40"
    >
      {props.children}
    </button>
  );
}

export function WindowControls() {
  const win = getCurrentWindow();
  return (
    <div class="flex">
      <Control label="Minimize" onClick={() => win.minimize()}><Minus size={14} stroke-width={1.5} /></Control>
      <Control label="Close" onClick={() => win.close()} danger><X size={16} stroke-width={1.5} /></Control>
    </div>
  );
}

/** `page` is the page (or sub-view) name; `feature` is the on/off setting key of the feature this page is (absent on non-feature pages). */
export function TitleBar(props: { title: string; page: string; feature?: string }) {
  return (
    <header data-tauri-drag-region class="flex h-10 shrink-0 items-center border-b border-line">
      <span data-tauri-drag-region class="t-label flex h-full w-[230px] shrink-0 items-center border-r border-line px-4 text-muted">{props.title}</span>
      <div data-tauri-drag-region class="flex flex-1 items-center pl-4">
        <Arrow label="Back" enabled={canBack()} onClick={back}><ChevronLeft size={16} stroke-width={1.5} /></Arrow>
        <Arrow label="Forward" enabled={canForward()} onClick={forward}><ChevronRight size={16} stroke-width={1.5} /></Arrow>
        <span data-tauri-drag-region class="t-label pl-3">{props.page}</span>
      </div>
      <Show when={props.feature} keyed>
        {(setting) => {
          // The feature's on/off, stored as `setting` and read by Rust (the shortcut engine and snippet expander follow it at once). `keyed` re-creates this per feature, so it always shows the right one.
          const [on, setOn] = useSetting(setting, true);
          return <div class="flex items-center pr-3"><Toggle label="Turn this feature on or off" checked={on()} onChange={setOn} /></div>;
        }}
      </Show>
      <WindowControls />
    </header>
  );
}
