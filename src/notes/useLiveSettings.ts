/**
 * File: notes/useLiveSettings.ts
 * Purpose: The notes settings and theme as they are now, for a note window that is already open: Rust pushes them whenever one is saved, and the page applies them at once (size, paper, colours, checked-item and number behaviour) with no reload.
 * Contents: parseLive — check the event's envelope before anything in it is read; useLiveSettings — the `options` signal, kept current (a malformed event is ignored); the size is set on the page here because CSS reads it.
 */
import { listen } from "@tauri-apps/api/event";
import { createSignal, onCleanup, onMount } from "solid-js";
import { isRecord } from "../ipc";
import { seedSettings } from "../settings";
import { isOptions, type Options } from "./types";

/** Sets the note text size on the page as the CSS variable the notes read. Input: the options. */
export const applySize = (o: Options) => document.documentElement.style.setProperty("--note-size", `${o.text_size}px`);

/** What a `notes-settings` event carries. */
type Live = { options: Options; theme: Record<string, unknown> };
/** The `notes-settings` payload, checked envelope first. Input: the raw payload. Return: the options and theme, or null when malformed. */
export const parseLive = (raw: unknown): Live | null =>
  isRecord(raw) && isOptions(raw.options) && isRecord(raw.theme) ? { options: raw.options, theme: raw.theme } : null;

/** Keeps the notes options and the theme keys current from the `notes-settings` event. Input: the options the page started with. Return: the options accessor. */
export function useLiveSettings(first: Options) {
  const [options, setOptions] = createSignal(first);
  onMount(() => {
    const off = listen<unknown>("notes-settings", ({ payload }) => {
      const live = parseLive(payload);
      if (!live) return;
      seedSettings(live.theme);
      applySize(live.options);
      setOptions(live.options);
    });
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  return options;
}
