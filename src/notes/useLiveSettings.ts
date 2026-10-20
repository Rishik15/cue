/**
 * Purpose: The notes settings and theme as they are now, for a note window that is already open: Rust pushes them whenever one is saved, and the page applies them at once (size, paper, colours, checked-item and number behaviour) with no reload.
 * Contents: useLiveSettings — the `options` signal, kept current; the size is set on the page here because CSS reads it.
 */
import { listen } from "@tauri-apps/api/event";
import { createSignal, onCleanup, onMount } from "solid-js";
import { seedSettings } from "../settings";
import { initTheme } from "../theme";
import { isOptions, type Options } from "./types";

export const applySize = (o: Options) => document.documentElement.style.setProperty("--note-size", `${o.text_size}px`);

export function useLiveSettings(first: Options) {
  const [options, setOptions] = createSignal(first);
  onMount(() => {
    const off = listen<{ options: unknown; theme: Record<string, unknown> }>("notes-settings", ({ payload }) => {
      if (!isOptions(payload.options)) return;
      seedSettings(payload.theme);
      initTheme();
      applySize(payload.options);
      setOptions(payload.options);
    });
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  return options;
}
