/**
 * Purpose: Navigation of the settings window: the page history behind the header's back and forward arrows, as in Raycast's settings.
 * Contents: current / canBack / canForward — state; detailTitle — the open sub-view's title; go — open a page; openDetail / closeDetail — a sub-view of the current page (the rule editor);
 * back / forward — move through the history. A sub-view is never kept for "forward": its state lives in its page and is gone once left.
 */
import { createSignal } from "solid-js";

/** The title of the current sub-view (the rule editor sets it), shown in the header where a hero page has no title. */
export const [detailTitle, setDetailTitle] = createSignal("");

type Entry = { page: string; detail: boolean };

const [stack, setStack] = createSignal<Entry[]>([{ page: "general", detail: false }]);
const [at, setAt] = createSignal(0);

export const current = () => stack()[at()];
export const canBack = () => at() > 0;
export const canForward = () => at() < stack().length - 1;

function push(entry: Entry) {
  // Leaving a sub-view for somewhere else drops it, so back never lands on a sub-view whose state is gone.
  const keep = current().detail && !entry.detail ? at() : at() + 1;
  setStack([...stack().slice(0, keep), entry]);
  setAt(keep);
}

export function go(page: string) {
  if (current().page === page && !current().detail) return;
  push({ page, detail: false });
}

export const openDetail = () => push({ page: current().page, detail: true });

export function back() {
  if (!canBack()) return;
  const leavingDetail = current().detail;
  setAt(at() - 1);
  if (leavingDetail) setStack(stack().slice(0, at() + 1));
}

export const closeDetail = () => current().detail && back();

export const forward = () => canForward() && setAt(at() + 1);
