/**
 * File: lookup/harness.ts
 * Purpose: The shared setup of the `useLookup` tests (Tauri's own `mockIPC`, the one allowed fake): mount the hook against a recorded backend, press keys in the search field or elsewhere, and settle a reply when a test decides.
 * Contents: SEARCH / ENTRY / entryOf / hit — sample payloads; later — a promise a test settles; open — mount the hook; keyFrom / press — keys; cleanup — dispose the mounted hooks and the mock.
 */
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { expect, vi } from "vitest";
import { useLookup } from "./useLookup";

export const SEARCH = { session: 4, mode: "search", theme: {}, recent: [{ word: "lucid", pos: "adjective" }], pack: true };
export const ENTRY = { key: "serene", language: "English", blocks: [{ word: "serene", pos: "adjective", senses: [{ gloss: "Calm." }] }] };
/** The sample entry under another key. Input: the key. Return: the entry. */
export const entryOf = (key: string) => ({ ...ENTRY, key });
/** A promise and the function that settles it, so a test decides when a reply arrives. */
export const later = <T>() => {
  let done!: (v: T) => void;
  let fail!: (e: string) => void;
  const promise = new Promise<T>((res, rej) => ((done = res), (fail = rej)));
  return { promise, done, fail };
};
/** A search hit for a word. Input: the word. Return: the hit. */
export const hit = (key: string) => ({ key, word: key, pos: "noun", gloss: `meaning of ${key}` });

/** The roots of the hooks a test mounted, disposed after it so no old page hears the next test's keys. */
export const disposers: (() => void)[] = [];

/** Mounts the hook against a backend that records the dictionary commands; `search` answers a query and `define` opens a word. Return: the hook and the recorded commands. */
export async function open(payload: unknown, search: (query: string) => unknown = () => [], define: (word: string) => unknown = () => ENTRY) {
  window.matchMedia = () => ({ matches: false }) as MediaQueryList;
  const calls: [string, unknown][] = [];
  mockIPC(
    (cmd, args) => {
      if (!cmd.startsWith("dict_")) return null;
      calls.push([cmd, args]);
      if (cmd === "dict_search") return search((args as { query: string }).query);
      if (cmd === "dict_define") return define((args as { word: string }).word);
      if (cmd === "dict_place") return { side: "centre", arrow: null, w: 360, h: 360 };
      return null;
    },
    { shouldMockEvents: true },
  );
  const s = createRoot((dispose) => {
    disposers.push(dispose);
    return useLookup();
  });
  await vi.waitFor(() => expect(calls.map((c) => c[0])).toContain("dict_ready"));
  await emit("dict-open", payload);
  return { s, calls };
}

/** The search field the keys come from, as the page's combobox input. */
const field = document.body.appendChild(document.createElement("input"));
field.setAttribute("role", "combobox");

/** Presses a key in an element; it bubbles to the window, where the page listens. Input: the element and the key. Return: the event, to see whether the page took it. */
export const keyFrom = (target: HTMLElement, key: string) => {
  const event = new KeyboardEvent("keydown", { key, cancelable: true, bubbles: true });
  target.dispatchEvent(event);
  return event;
};

/** Presses a key in the search field. Input: the key. */
export const press = (key: string) => keyFrom(field, key);


/** Disposes the hooks a test mounted and removes the mock; call it from `afterEach`. */
export async function cleanup() {
  disposers.splice(0).forEach((d) => d());
  await new Promise((r) => setTimeout(r, 0));
  clearMocks();
}
