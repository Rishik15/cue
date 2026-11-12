/**
 * File: strip/StripApp.tsx
 * Purpose: The Transform Text window's page: one floating card near the selected text that is the strip, then the command list, then the result. The side (below or above the text) is chosen once per open; the card is pinned to the window's corner next to the text, and the window never moves that corner, so the card stays put and grows away from the text. Each time the view, its outcome or its size changes the new view is simply there and the card eases to its size (`useMorph`). The list and result code load after the strip is laid out, through each lazy view's `preload`, so a view's first render is synchronous and its size is measured in the same turn.
 * Contents: StripApp — waits for the open event; Stage — the card and its placement; Views — the content of each view.
 */
import { createEffect, createMemo, lazy, Match, on, Show, Suspense, Switch, untrack } from "solid-js";
import { CardView, FloatingCard } from "../ui/FloatingCard";
import { StripBar } from "../ui/Strip";
import { key, query, type View } from "./flow";
import { MARGIN, room, side } from "./place";
import type { Open } from "./payload";
import { useMorph } from "./useMorph";
import { useStrip, type Strip } from "./useStrip";

const ListView = lazy(() => import("./ListView"));
const ResultView = lazy(() => import("./ResultView"));

/** The size of each view's content. */
const SIZE = { strip: "fit", list: "list", result: "panel" } as const;

/** The card and its views for one open strip. Input: the open payload and the page state. */
function Stage(p: { open: Open; s: Strip }) {
  const where = untrack(() => side(p.open.anchor, p.open.work));
  const m = untrack(() => useMorph(p.open, where));
  /** The flow, if the open event has started one. */
  const flow = () => p.s.flow();
  /** What decides the card's size: the view, the outcome and the row. */
  const shape = createMemo(() => `${flow()?.view}:${flow()?.outcome.kind}:${flow()?.row}`);
  createEffect(on(shape, m.schedule));
  /** Whether the card has its first size: the strip is laid out. */
  const placed = createMemo(() => m.box() !== undefined);
  createEffect(on(placed, (now) => now && void Promise.all([ListView.preload(), ResultView.preload()])));
  /** The view on screen. */
  const view = () => flow()?.view ?? "strip";
  return (
    <div class="h-full w-full" onMouseDown={(e) => e.target === e.currentTarget && p.s.close()}>
      <div class="absolute" style={{ left: `${MARGIN}px`, top: where === "below" ? `${MARGIN}px` : undefined, bottom: where === "above" ? `${MARGIN}px` : undefined, "--room": `${room(p.open.anchor, p.open.work, where)}px`, "--panel": `${m.panel()}px` }}>
        <FloatingCard side={where} box={m.box()} hidden={m.hidden()} closing={p.s.closing()} label="Transform text">
          <Show when={view()} keyed>
            {(v) => (
              <CardView size={SIZE[v]} side={where} morph ref={m.content}>
                <Views s={p.s} view={v} />
              </CardView>
            )}
          </Show>
        </FloatingCard>
      </div>
    </div>
  );
}

/** The content of the card for one view, kept for that view (so a view on its way out still shows what it showed); the list and result arrive as lazy chunks. Input: the page state and the view. */
function Views(p: { s: Strip; view: View }) {
  /** The flow. */
  const f = () => p.s.flow();
  /** The chips as the strip shows them. */
  const chips = () => f()?.chips.map((row) => ({ name: f()?.rows[row]?.name ?? "", model: f()?.rows[row]?.model ?? false })) ?? [];
  /** Presses key `k` in the flow. */
  const press = (k: string) => p.s.act((now) => key(now, k));
  return (
    <Suspense>
      <Switch>
        <Match when={p.view === "strip"}>
          <StripBar chips={chips()} onRun={(i) => p.s.choose(f()?.chips[i] ?? -1)} onMore={() => p.s.act((now) => [query(now, ""), { kind: "none" }])} />
        </Match>
        <Match when={p.view === "list" && f()}>
          {(now) => <ListView flow={now()} onType={p.s.type} onHover={p.s.point} onPick={p.s.choose} />}
        </Match>
        <Match when={p.view === "result" && f()}>
          {(now) => <ResultView flow={now()} slow={p.s.slow()} onKey={press} />}
        </Match>
      </Switch>
    </Suspense>
  );
}

/** The window's page: nothing until the open event arrives (the window is hidden until the card is measured), then the stage. */
export function StripApp() {
  const s = useStrip();
  return <Show when={s.open()}>{(o) => <Stage open={o()} s={s} />}</Show>;
}
