/**
 * File: notes/usePins.ts
 * Purpose: Pinning a note with the answers kept straight: the open note shows the wanted state at once and, once every ask for it is answered, what Rust stored.
 * Contents: usePins - returns `pin`.
 */
import { callNote } from "./api";
import type { Open } from "./types";

/** Pinning with the answers kept straight. Input: the open note signal and its setter, the id of the note on screen, the queue and where a problem goes. Return: `pin`. */
export function usePins(open: () => Open, setOpen: (f: (o: Open) => Open) => void, id: () => number, enqueue: <T,>(job: () => Promise<T>) => Promise<T>, fail: (e: unknown) => void) {
  /** What is stored for each note with a pin in flight: the state Rust last accepted (unknown for a note not on screen until it is opened or an ask is accepted) and how many asks are still out. */
  const pins = new Map<number, { stored: boolean | undefined; left: number }>();
  /** Pins or unpins a note; the open note shows it at once, and once every ask for it is answered it shows what Rust stored. Input: the note and the wanted state. Return: nothing. */
  const pin = (target: number, pinned: boolean) => {
    /** Shows a pinned state on the note if it is the one open. Input: the state. Return: nothing. */
    const show = (value: boolean) => setOpen((o) => (o.note.id === target ? { ...o, note: { ...o.note, pinned: value } } : o));
    const entry = pins.get(target) ?? { stored: undefined, left: 0 };
    if (entry.stored === undefined && target === id()) entry.stored = open().note.pinned;
    pins.set(target, entry);
    entry.left++;
    show(pinned);
    return enqueue(async () => {
      try {
        await callNote({ do: "pin", id: target, pinned });
        entry.stored = pinned;
      } catch (e) {
        fail(e);
      }
      if (--entry.left === 0) {
        pins.delete(target);
        if (entry.stored !== undefined) show(entry.stored);
      }
    });
  };
  return pin;
}
