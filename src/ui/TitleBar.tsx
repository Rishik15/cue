/**
 * Purpose: Full-width header band split like the body: app title over the sidebar, page name over the content, window controls at right.
 * Contents: TitleBar — slim header for the content column; Control — one caption button.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-solid";
import { createSignal, onCleanup, onMount, type JSX } from "solid-js";

function Control(props: { label: string; onClick: () => void; danger?: boolean; children: JSX.Element }) {
  return (
    <button
      aria-label={props.label}
      onClick={() => props.onClick()}
      class="grid h-[39px] w-[46px] place-items-center text-muted"
      classList={{ "hover:bg-danger hover:text-white": props.danger, "hover:bg-hover hover:text-ink": !props.danger }}
    >
      {props.children}
    </button>
  );
}

export function TitleBar(props: { title: string; page: string }) {
  const win = getCurrentWindow();
  const [maximized, setMaximized] = createSignal(false);
  onMount(() => {
    const sync = () => win.isMaximized().then(setMaximized);
    sync();
    const unlisten = win.onResized(sync);
    onCleanup(() => unlisten.then((f) => f()));
  });
  return (
    <header data-tauri-drag-region class="flex h-10 shrink-0 items-center border-b border-line">
      <span data-tauri-drag-region class="t-label flex h-full w-[230px] shrink-0 items-center border-r border-line px-4 text-muted">{props.title}</span>
      <span data-tauri-drag-region class="t-label flex-1 px-4">{props.page}</span>
      <div class="flex">
        <Control label="Minimize" onClick={() => win.minimize()}><Minus size={14} /></Control>
        <Control label={maximized() ? "Restore" : "Maximize"} onClick={() => win.toggleMaximize()}>{maximized() ? <Copy size={12} /> : <Square size={11} />}</Control>
        <Control label="Close" onClick={() => win.close()} danger><X size={15} /></Control>
      </div>
    </header>
  );
}
