/**
 * File: route.ts
 * Purpose: The door for a page name that arrives from outside the window (Rust's `cue-goto` event and `take_start_page`): a known page opens, an unknown name is refused and reported, so the window never sits on a page it cannot draw.
 * Contents: goTo — open the page a name stands for; useStartPage — follow Rust's request to open a page.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { onCleanup, onMount } from "solid-js";
import { go } from "./nav";
import { parsePage } from "./pages/catalog";
import { showToast } from "./ui/Toast";

/** Opens the page `name` stands for, or tells the user and stays where it is when the name is not a page. Input: whatever arrived. */
export function goTo(name: unknown) {
  const page = parsePage(name);
  if (page) go(page);
  else showToast(`Cue could not open the page "${String(name)}".`);
}

/** Follows Rust's request to open a page (About, when an update waits). Rust keeps the page until the window takes it: asked for once the listener exists, and again on each `cue-goto`. */
export function useStartPage() {
  onMount(() => {
    const takePage = () => void invoke<unknown>("take_start_page").then((page) => page && goTo(page), () => undefined);
    const off = listen("cue-goto", takePage);
    void off.then(takePage, () => undefined);
    onCleanup(() => void off.then((unlisten) => unlisten()));
  });
}
