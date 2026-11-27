/**
 * File: ui/WindowBoundary.tsx
 * Purpose: The error boundary every webview renders inside: a page that throws closes its window once, so no window is left frozen on top of everything.
 * Contents: quietly — run a window call that may fail; WindowBoundary — the boundary that closes the window once.
 */
import { ErrorBoundary, type JSX, Show } from "solid-js";

/** Runs a window call and drops its failure (a window that is already gone). Input: the call. */
export const quietly = (call: () => unknown) => void Promise.resolve().then(call).catch(() => undefined);

/** Renders the page; a page that throws closes the window once and shows only `message`, if given. Input: close, an optional message and the page. Return: the boundary. */
export function WindowBoundary(p: { close: () => unknown; message?: string; children: JSX.Element }) {
  let closed = false;
  return (
    <ErrorBoundary fallback={() => {
      if (!closed) quietly(p.close);
      closed = true;
      return <Show when={p.message}>{(m) => <p class="t-hint p-4">{m()}</p>}</Show>;
    }}>
      {p.children}
    </ErrorBoundary>
  );
}
