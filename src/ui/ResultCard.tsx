/**
 * File: ui/ResultCard.tsx
 * Purpose: The Transform Text result card, laid out like a settings card's rows divided by hairlines: the change as a word diff of the original against the result (removed words muted with a thin strike, added words on a faint wash, as `<del>` and `<ins>`), or the plain result; then the shared `ActionFooter` after Raycast: the action's name and a muted note at the left, Retry and Reject (or Back) as muted text actions with their caps, a divider, and Paste in `ink` at the far right. A text-model run says its name and how many words it works on beside the settings window's wave of five bars; no change, and a run that did not finish, say so. Every text line starts 12 from the card edge, and the last key cap ends 12 from it.
 * Contents: ResultState — what the card shows; ResultCard; the note, change, diff, message, working, body and outcome parts.
 */
import { createMemo, For, Match, Show, Switch } from "solid-js";
import { analyse, wordsIn, type Part, type Shown } from "./diff";
import { ActionFooter } from "./ActionFooter";

/** What the card shows: a run still working (`slow` after a while), no change, a run that did not finish, or the result. */
export type ResultState = { kind: "working"; slow: boolean } | { kind: "unchanged" } | { kind: "failed" } | { kind: "done"; text: string };

/** The header note: how many changes the diff marks, or the word counts for a plain result; while working, how to cancel. Input: the original, the result (null while there is none), how it is shown and whether it works. Return: the note. */
function note(original: string, text: string | null, shown: Shown | null, working: boolean): string {
  if (working) return "Esc cancels";
  if (text === null || !shown) return "";
  if (shown.mode === "diff") return `${shown.changes} ${shown.changes === 1 ? "change" : "changes"}`;
  if (shown.mode === "none") return "No changes";
  return `${wordsIn(original)} to ${wordsIn(text)} words`;
}

/** One changed stretch as `<del>` or `<ins>`, its leading and trailing spaces left outside the mark so a strike or tint never runs into the space. Input: the tag, the class and the text. */
function Change(p: { tag: "del" | "ins"; class: string; text: string }) {
  /** The text as its leading space, its words and its trailing space. */
  const cut = () => /^(\s*)([\s\S]*?)(\s*)$/.exec(p.text) ?? ["", "", p.text, ""];
  return (
    <Show when={cut()[2]}>
      {cut()[1]}
      {p.tag === "del" ? <del class={p.class}>{cut()[2]}</del> : <ins class={p.class}>{cut()[2]}</ins>}
      {cut()[3]}
    </Show>
  );
}

/** The diff: unchanged runs as text, each change as its removed then its added stretch. Input: the parts. */
const Diff = (p: { parts: Part[] }) => (
  <For each={p.parts}>
    {(part) =>
      "same" in part ? (
        part.same
      ) : (
        <>
          <Change tag="del" class="diff-del" text={part.del} />
          <Change tag="ins" class="diff-add" text={part.ins} />
        </>
      )
    }
  </For>
);

/** A short message in the body, a title line over a muted hint line. Input: both lines. */
const Message = (p: { title: string; hint: string }) => (
  <div class="flex flex-col gap-0.5 px-3 py-2.5">
    <p class="t-label">{p.title}</p>
    <p class="t-hint">{p.hint}</p>
  </div>
);

/** The model at work: the settings window's wave of five bars beside the name and word count (a reassurance once it is slow). Input: the title, the original and whether it is slow. */
const Working = (p: { title: string; original: string; slow: boolean }) => (
  <div class="flex items-center gap-3 px-3 py-2.5 text-muted">
    <span class="flex h-4 w-[27px] shrink-0 items-center justify-between" aria-hidden="true">
      <For each={[0, 1, 2, 3, 4]}>{(i) => <span class="wave-bar" style={{ "animation-delay": `${i * 120}ms` }} />}</For>
    </span>
    <p class="t-hint">{p.slow ? `${p.title}: still working. This model is slow on long text. Esc cancels.` : `${p.title}: working on ${wordsIn(p.original)} words…`}</p>
  </div>
);

/** The result text, 12 from the card's sides, scrolling past thirteen lines; selectable so it can be read and copied. Input: the text and how it is shown. */
const Body = (p: { text: string; shown: Shown }) => (
  <div class="thin-scroll t-label max-h-[240px] select-text overflow-y-auto whitespace-pre-wrap break-words px-3 py-2.5 [overflow-wrap:anywhere]">
    <Show when={p.shown.mode === "diff" && p.shown} fallback={p.text}>{(d) => <Diff parts={d().parts} />}</Show>
  </div>
);


/** The body for each state, in one live region so the outcome is announced. Input: the title, the state, whether it is a model action, the original, the result text and how it is shown. */
function Outcome(p: { title: string; state: ResultState; model: boolean; original: string; done: string | null; shown: Shown | null }) {
  return (
    <div role="status" class="flex min-h-0 flex-col">
      <Switch>
        <Match when={p.state.kind === "working" && p.state}>{(w) => <Working title={p.title} original={p.original} slow={w().slow} />}</Match>
        <Match when={p.state.kind === "unchanged"}>
          <Message title="Nothing to change." hint={p.model ? "The model left this text as it was." : "This text is already how this action leaves it."} />
        </Match>
        <Match when={p.state.kind === "failed"}>
          <Message title={p.model ? "The model did not finish." : "The action did not finish."} hint="Your text was not changed." />
        </Match>
        <Match when={p.done !== null}>
          <Body text={p.done ?? ""} shown={p.shown ?? { mode: "plain" }} />
        </Match>
      </Switch>
    </div>
  );
}

/** The card for one action's result. Input: the action's name, whether it runs the model, the original, the state, and the actions (Retry only for a model action). */
export function ResultCard(p: { title: string; model: boolean; original: string; state: ResultState; onPaste: () => void; onReject: () => void; onRetry?: () => void }) {
  /** The result text, when there is one. */
  const done = createMemo(() => (p.state.kind === "done" ? p.state.text : null));
  /** How the result is shown, when there is one. */
  const shown = createMemo(() => {
    const text = done();
    return text === null ? null : analyse(p.original, text);
  });
  return (
    <div class="flex w-full flex-col">
      <Outcome title={p.title} state={p.state} model={p.model} original={p.original} done={done()} shown={shown()} />
      <Show when={p.state.kind !== "working"}>
        <ActionFooter
          title={
            <>
              {p.title}
              <span class="ml-2">{note(p.original, done(), shown(), false)}</span>
            </>
          }
          secondary={[
            ...(p.state.kind !== "unchanged" && p.onRetry ? [{ label: "Retry", keys: ["R"], onClick: () => p.onRetry?.() }] : []),
            ...(done() !== null ? [{ label: "Reject", keys: ["Esc"], onClick: p.onReject }] : []),
          ]}
          primary={done() !== null ? { label: "Paste", keys: ["↵"], onClick: p.onPaste } : { label: "Back", keys: ["Esc"], onClick: p.onReject }}
        />
      </Show>
    </div>
  );
}
