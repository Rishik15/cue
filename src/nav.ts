/**
 * File: nav.ts
 * Purpose: Navigation of the settings window: the page history behind the header's back and forward arrows.
 * Contents: current / canBack / canForward — state; detailTitle — the open sub-view's title; go — open a page; openDetail / closeDetail — a sub-view of the current page (the rule editor);
 * back / forward — move through the history. A sub-view is never kept for "forward": its state lives in its page and is gone once left.
 */
import { batch, createSignal } from "solid-js";
import type { PageId } from "./pages/catalog";

/** The title of the current sub-view (the rule editor sets it), shown in the header where a hero page has no title. */
export const [detailTitle, setDetailTitle] = createSignal("");

type Entry = { page: PageId; detail: boolean };

const [stack, setStack] = createSignal<Entry[]>([{ page: "general", detail: false }]);
const [at, setAt] = createSignal(0);

const FIRST: Entry = { page: "general", detail: false };
export const current = () => stack()[at()] ?? FIRST;
export const canBack = () => at() > 0;
export const canForward = () => at() < stack().length - 1;

/** Adds `entry` to the history (dropping a sub-view being left). Input: the entry. */
function push(entry: Entry) {
  // Leaving a sub-view for somewhere else drops it, so back never lands on a sub-view whose state is gone.
  const keep = current().detail && !entry.detail ? at() : at() + 1;
  // One update: between the two setters `current()` would be the old entry, and the rule list's effect would read that as "left the editor".
  batch(() => {
    setStack([...stack().slice(0, keep), entry]);
    setAt(keep);
  });
}

/** Opens a page unless it is already the open one. Input: a page id (names from outside go through `parsePage` first). */
export function go(page: PageId) {
  if (current().page === page && !current().detail) return;
  push({ page, detail: false });
}

/** Opens a sub-view of the current page (the rule editor). */
export const openDetail = () => push({ page: current().page, detail: true });

/** Goes one entry back in the history, if there is one. */
export function back() {
  if (!canBack()) return;
  const leavingDetail = current().detail;
  batch(() => {
    setAt(at() - 1);
    if (leavingDetail) setStack(stack().slice(0, at() + 1));
  });
}

/** Leaves the open sub-view, if any. */
export const closeDetail = () => current().detail && back();

/** Goes one entry forward in the history, if there is one. */
export const forward = () => canForward() && setAt(at() + 1);
