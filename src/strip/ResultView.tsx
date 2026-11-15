/**
 * File: strip/ResultView.tsx
 * Purpose: The Transform Text result, loaded after the strip is on screen: the chosen action's change with Paste, Reject and (for a text-model action) Retry. Assembled from `ui/ResultCard`; its buttons press the same keys the keyboard does, so clicks and keys take one path through the flow.
 * Contents: ResultView.
 */
import { Show } from "solid-js";
import { diffsFor } from "../ui/diff";
import { ResultCard, type ResultState } from "../ui/ResultCard";
import type { Flow } from "./flow";

/** The result view. Input: the flow, whether a model run is slow and what a key press does. */
export default function ResultView(p: { flow: Flow; slow: boolean; onKey: (key: string) => void }) {
  /** The row the result is for. */
  const row = () => (p.flow.row === null ? undefined : p.flow.rows[p.flow.row]);
  /** The card's state: the outcome, with whether a working run is slow. */
  const state = (): ResultState => {
    const o = p.flow.outcome;
    return o.kind === "working" ? { kind: "working", slow: p.slow } : o;
  };
  return (
    <Show when={row()}>
      {(r) => (
        <ResultCard
          title={r().name}
          model={r().model}
          diff={diffsFor(r().id, r().group)}
          original={p.flow.original}
          state={state()}
          compare={p.flow.compare}
          onCompare={() => p.onKey("o")}
          onPaste={() => p.onKey("Enter")}
          onReject={() => p.onKey("Escape")}
          onRetry={r().model ? () => p.onKey("r") : undefined}
        />
      )}
    </Show>
  );
}
