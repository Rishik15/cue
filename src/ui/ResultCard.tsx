/**
 * File: ui/ResultCard.tsx
 * Purpose: The Transform Text result card: the user's own text or its change on one raised plate over the shared `ActionFooter` (Raycast's bar). The change is a word diff of the original against the result (removed words muted with a thin strike, added words on a faint wash, as `<del>` and `<ins>`) for the actions whose result is the same text with a few words fixed (`diffsFor`), else the plain result. A text-model run shows the original on its plate with a sweep across it, the action's name and how many words it works on; no change and a run that did not finish keep the original in view and say so. The footer has the action's name and a muted note, Retry and Reject (or Back) as muted text actions with their caps, a divider, and Paste in `ink` at the far right.
 * Contents: ResultState — what the card shows; ResultCard; the note, change, diff, original plate, body, header, working, failed and outcome parts.
 */
import { Type } from "lucide-solid";
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

/** The user's own text on a plate, three lines at most, muted: what the action is working on or what it left alone. Input: the text, and whether the working sweep crosses it. */
const Original = (p: { text: string; sweep?: boolean }) => (
  <div class="plate relative overflow-hidden px-3 py-2.5">
    <Show when={p.sweep}><span class="sweep" aria-hidden="true" /></Show>
    <p class="t-label relative line-clamp-3 break-words text-muted [overflow-wrap:anywhere]">{p.text}</p>
  </div>
);

/** The result text on a plate, scrolling past thirteen lines; selectable so it can be read and copied. Input: the text and how it is shown. */
const Body = (p: { text: string; shown: Shown }) => (
  <div class="p-2">
    <div class="plate thin-scroll t-label max-h-[240px] select-text overflow-y-auto whitespace-pre-wrap break-words px-3 py-2.5 [overflow-wrap:anywhere]">
      <Show when={p.shown.mode === "diff" && p.shown} fallback={p.text}>{(d) => <Diff parts={d().parts} />}</Show>
    </div>
  </div>
);

/** The line above a result: the action's name at the left, what it did (the count of changes) at the right. Input: both. */
const Head = (p: { title: string; note: string }) => (
  <div class="flex items-baseline justify-between gap-3 px-5 pt-3">
    <p class="t-control min-w-0 truncate">{p.title}</p>
    <p class="t-hint shrink-0">{p.note}</p>
  </div>
);

/** The model at work: the action's name and how to stop it above the user's text on its plate with the sweep across it; once it is slow the note says it is still going. Input: the title, the original and whether it is slow. */
const Working = (p: { title: string; original: string; slow: boolean }) => (
  <>
    <Head title={p.title} note={p.slow ? "Still working" : "Esc to cancel"} />
    <div class="p-2">
      <Original text={p.original} sweep />
    </div>
  </>
);

/** A run that did not finish: the name and what happened above the user's text, with a line saying it is unchanged. Input: the title and the original. */
const Failed = (p: { title: string; original: string }) => (
  <>
    <Head title={p.title} note="Could not finish" />
    <div class="p-2">
      <Original text={p.original} />
      <p class="t-hint px-3 pb-1 pt-2">Your text is unchanged.</p>
    </div>
  </>
);

/** The body for each state, in one live region so the outcome is announced. Input: the title, the note, the state, the original, the result text and how it is shown. */
function Outcome(p: { title: string; note: string; state: ResultState; original: string; done: string | null; shown: Shown | null; compare: boolean }) {
  return (
    <div role="status" class="flex min-h-0 flex-col">
      <Switch>
        <Match when={p.state.kind === "working" && p.state}>{(w) => <Working title={p.title} original={p.original} slow={w().slow} />}</Match>
        <Match when={p.state.kind === "unchanged"}>
          <Head title={p.title} note="No changes" />
          <Body text={p.original} shown={{ mode: "plain" }} />
        </Match>
        <Match when={p.state.kind === "failed"}>
          <Failed title={p.title} original={p.original} />
        </Match>
        <Match when={p.done !== null}>
          <Head title={p.title} note={p.compare ? "Original" : p.note} />
          <Body text={p.compare ? p.original : (p.done ?? "")} shown={p.compare ? { mode: "plain" } : (p.shown ?? { mode: "plain" })} />
        </Match>
      </Switch>
    </div>
  );
}

/** The card for one action's result. Input: the action's name, whether its result is shown as a diff (`diffsFor`), the original, the state, whether the original is shown in place of the result (`compare`), and the actions (Retry only for a model action; Original or Result swaps what the plate shows). */
export function ResultCard(p: { title: string; diff: boolean; original: string; state: ResultState; compare: boolean; onPaste: () => void; onReject: () => void; onRetry?: () => void; onCompare: () => void }) {
  /** The result text, when there is one. */
  const done = createMemo(() => (p.state.kind === "done" ? p.state.text : null));
  /** How the result is shown, when there is one. */
  const shown = createMemo((): Shown | null => {
    const text = done();
    if (text === null) return null;
    return p.diff ? analyse(p.original, text) : p.original === text ? { mode: "none" } : { mode: "plain" };
  });
  return (
    <div class="flex w-full flex-col">
      <Outcome title={p.title} note={note(p.original, done(), shown(), false)} state={p.state} original={p.original} done={done()} shown={shown()} compare={p.compare} />
      <Show when={p.state.kind !== "working"}>
        <ActionFooter
          icon={<Type size={15} stroke-width={2} aria-hidden="true" />}
          secondary={[
            ...(done() !== null ? [{ label: p.compare ? "Result" : "Original", keys: ["O"], onClick: p.onCompare }] : []),
            ...(p.state.kind !== "unchanged" && p.onRetry ? [{ label: "Retry", keys: ["R"], onClick: () => p.onRetry?.() }] : []),
            ...(done() !== null ? [{ label: "Reject", keys: ["Esc"], onClick: p.onReject }] : []),
          ]}
          primary={done() !== null ? { label: "Paste", keys: ["↵"], onClick: p.onPaste } : { label: "Back", keys: ["Esc"], onClick: p.onReject }}
        />
      </Show>
    </div>
  );
}
