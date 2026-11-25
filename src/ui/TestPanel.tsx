/**
 * File: ui/TestPanel.tsx
 * Purpose: Live dry run for the rule being edited: type a sample text and see, as you type, which rules match and the exact result.
 * Touches nothing. Runs a moment after the last change; steps that use the text model run only when asked (a button), since they take seconds.
 * Contents: TestPanel — sample input, a pretend program and the outcome (a reply to an older input is ignored); Outcome — the result block.
 */
import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import { testRules, type ActionInfo, type Rule, type TestResult } from "../rules";
import { Button, Card, Hint, Row, Section, TextArea, TextField } from "./kit";
import { latest } from "./latest";

const DELAY_MS = 250;
const GOES_TO = { copy: "the clipboard", preview: "the preview", insert: "the cursor" };

/** The result block. Input: the test result. Return: the block. */
function Outcome(p: { result: TestResult }) {
  return (
    <div class="flex flex-col gap-2 px-3 py-[13px]">
      <div class="t-label">Result</div>
      <pre class="t-control max-h-40 min-h-8 overflow-auto whitespace-pre-wrap break-words rounded-md bg-control px-2.5 py-1.5 font-[inherit] select-text">{p.result.text}</pre>
      <p class="t-hint">
        <Show when={p.result.matched.length} fallback="No action matched this text.">
          Matched <For each={p.result.matched}>{(name, i) => <>{i() > 0 && ", "}{name}</>}</For>. The result goes to {GOES_TO[p.result.output]}.
        </Show>
      </p>
    </div>
  );
}

/** The live test. `initial` is the sample the panel opens with (a typical text for the rule's first action), so it never starts empty. */
export function TestPanel(p: { rules: () => Rule[]; trigger: () => Rule["trigger"]; initial: string; actions: ActionInfo[] }) {
  const [sample, setSample] = createSignal(p.initial);
  const skipsModel = () => p.rules().some((r) => r.actions.some((id) => p.actions.find((a) => a.id === id)?.model));
  const [app, setApp] = createSignal("");
  const [result, setResult] = createSignal<TestResult | null>(null);
  const [error, setError] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  // Every request and every change of input is a new request; a reply shows only while it is still the latest, so a slow old reply never overwrites the result of newer input.
  const request = latest();
  /** Shows the reply of a request unless newer input has come since. Input: the request's check and its reply. Return: a promise that settles once shown or dropped. */
  const show = (mine: () => boolean, run: Promise<TestResult>) =>
    run.then((r) => { if (mine()) { setResult(r); setError(""); } }, (e) => { if (mine()) { setResult(null); setError(String(e)); } });
  createEffect(() => {
    const [rules, trigger, text, program] = [p.rules(), p.trigger(), sample(), app()];
    const mine = request();
    if (!text) return setResult(null);
    const timer = setTimeout(() => void show(mine, testRules(rules, trigger, text, program)), DELAY_MS);
    onCleanup(() => clearTimeout(timer));
  });
  const runModel = () => {
    setBusy(true);
    void show(request(), testRules(p.rules(), p.trigger(), sample(), app(), true)).finally(() => setBusy(false));
  };
  return (
    <Section title="Test">
      <Card>
        <div class="flex flex-col gap-2 px-3 py-[13px]">
          <div class="t-label">Sample Text</div>
          <TextArea label="Sample text" placeholder="Paste or type some text to see what your actions do with it" value={sample()} onInput={setSample} />
        </div>
        <Row label="Program" hint="For actions limited to programs"><div class="w-40"><TextField label="Program" placeholder="slack" value={app()} onInput={setApp} /></div></Row>
        <Show when={skipsModel()}>
          <Row label="Text Model Steps" hint="Not run in the live result">
            <Button onClick={runModel} reserve={["Running…", "Run With Text Model"]}>{busy() ? "Running…" : "Run With Text Model"}</Button>
          </Row>
        </Show>
        <Show when={error()}><Hint danger>{error()}</Hint></Show>
        <Show when={result()}>{(r) => <Outcome result={r()} />}</Show>
      </Card>
    </Section>
  );
}
