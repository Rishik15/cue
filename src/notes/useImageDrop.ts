/**
 * File: useImageDrop.ts
 * Purpose: Picture drops on the note window: a picture dragged over shows a hint, dropped it is read with Windows' text recognition (Rust, `Cmd::Image`) and the text is added to the note.
 * Contents: useImageDrop — the drop state ("over" while a picture hovers, "reading" while Rust works) and the one listener for the window's life.
 */
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { createSignal, onCleanup, onMount } from "solid-js";
import { callNote } from "./api";

const PICTURE = /\.(png|jpe?g|bmp|gif|tiff?|webp)$/i;
export type Drop = "idle" | "over" | "reading";

/** The picture drop listener. Input: where the recognised text goes, where a problem goes, and `hold`, which runs a job in the note's queue so a move or close waits until the text is in the note it came from. Return: the drop state. */
export function useImageDrop(onText: (text: string) => void, onProblem: (message: string) => void, hold: (job: () => Promise<void>) => Promise<void>) {
  const [drop, setDrop] = createSignal<Drop>("idle");
  /** Not the signal: the listener must not read reactive state. */
  let busy = false;
  /** Reads one picture inside the note's queue. Input: the picture's path. Return: nothing. */
  const read = (path: string) => {
    busy = true;
    setDrop("reading");
    return hold(async () => {
      const text = await callNote({ do: "image", path }).catch((e: unknown) => onProblem(String(e)));
      busy = false;
      setDrop("idle");
      if (typeof text === "string") onText(text);
    });
  };
  onMount(() => {
    const off = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (busy) return;
      const picture = "paths" in payload ? payload.paths.find((p) => PICTURE.test(p)) : undefined;
      if (payload.type === "drop" && picture) void read(picture);
      else setDrop(payload.type !== "leave" && payload.type !== "drop" && picture ? "over" : "idle");
    });
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
  return drop;
}
