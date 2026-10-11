/**
 * Purpose: Live dry run for the rule being edited: type a sample text and see, as you type, which rules match and the exact result.
 * Touches nothing. Runs a moment after the last change.
 * Contents: TestPanel — sample input, a pretend program and the outcome.
 */
import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import { testRules, type ActionInfo, type Rule, type TestResult } from "../rules";
import { Card, Row, Section, TextArea, TextField } from "./kit";

const DELAY_MS = 250;
const GOES_TO = { copy: "the clipboard", preview: "the preview", insert: "the cursor" };

/** `initial` is the sample the panel opens with (a typical text for the rule's first action), so it never starts empty. */
export function TestPanel(p: { rules: () => Rule[]; trigger: () => Rule["trigger"]; initial: string; actions: ActionInfo[] }) {
  const [sample, setSample] = createSignal(p.initial);
  const skipsModel = () => p.rules().some((r) => r.actions.some((id) => p.actions.find((a) => a.id === id)?.model));
  const [app, setApp] = createSignal("");
  const [result, setResult] = createSignal<TestResult | null>(null);
  const [error, setError] = createSignal("");
  createEffect(() => {
    const [rules, trigger, text, program] = [p.rules(), p.trigger(), sample(), app()];
    if (!text) return setResult(null);
    const timer = setTimeout(() => {
      testRules(rules, trigger, text, program).then(
        (r) => { setResult(r); setError(""); },
        (e) => { setResult(null); setError(String(e)); },
      );
    }, DELAY_MS);
    onCleanup(() => clearTimeout(timer));
  });
  return (
    <Section title="Test">
      <Card>
        <div class="flex flex-col gap-2 px-3 py-[13px]">
          <div class="t-label">Sample Text</div>
          <TextArea label="Sample text" placeholder="Paste or type a text to see what your rules do with it" value={sample()} onInput={setSample} />
        </div>
        <Row label="Program" hint="Only needed for rules limited to programs."><div class="w-40"><TextField label="Program" placeholder="slack" value={app()} onInput={setApp} /></div></Row>
        <Show when={skipsModel()}><p class="t-hint px-3 py-[13px]">Actions that use the text model are not run in this test, so the result leaves their step out.</p></Show>
        <Show when={error()}><p class="t-hint px-3 py-[13px] !text-danger">{error()}</p></Show>
        <Show when={result()}>
          {(r) => (
            <div class="flex flex-col gap-2 px-3 py-[13px]">
              <div class="t-label">Result</div>
              <pre class="t-control max-h-40 min-h-8 overflow-auto whitespace-pre-wrap break-words rounded-md bg-control px-2.5 py-1.5 font-[inherit] select-text">{r().text}</pre>
              <p class="t-hint">
                <Show when={r().matched.length} fallback="No rule matched this text.">
                  Matched <For each={r().matched}>{(name, i) => <>{i() > 0 && ", "}{name}</>}</For>. The result goes to {GOES_TO[r().output]}.
                </Show>
              </p>
            </div>
          )}
        </Show>
      </Card>
    </Section>
  );
}
