/**
 * Purpose: Dry run for a rule being edited: type a sample text, see which rules match and the exact result, without touching anything.
 * Contents: TestPanel — sample input, a pretend program, the run button and the outcome.
 */
import { createSignal, For, Show } from "solid-js";
import { testRules, type Rule, type TestResult } from "../rules";
import { Button, Card, Row, Section, TextArea, TextField } from "./kit";

export function TestPanel(p: { rules: () => Rule[]; trigger: () => Rule["trigger"] }) {
  const [sample, setSample] = createSignal("");
  const [app, setApp] = createSignal("");
  const [result, setResult] = createSignal<TestResult | null>(null);
  const [error, setError] = createSignal("");
  const run = () =>
    testRules(p.rules(), p.trigger(), sample(), app()).then(
      (r) => { setResult(r); setError(""); },
      (e) => { setResult(null); setError(String(e)); },
    );
  return (
    <Section title="Test">
      <Card>
        <div class="flex flex-col gap-2 px-3 py-[13px]">
          <TextArea label="Sample text" placeholder="Paste or type a sample text" value={sample()} onInput={setSample} />
        </div>
        <Row label="Program" hint="Only needed for rules limited to programs."><div class="w-40"><TextField label="Program" placeholder="slack" value={app()} onInput={setApp} /></div></Row>
        <Row label="Run all enabled rules on this text"><Button onClick={run}>Run Test</Button></Row>
        <Show when={error()}><p class="t-hint px-3 py-[13px] !text-danger">{error()}</p></Show>
        <Show when={result()}>
          {(r) => (
            <div class="flex flex-col gap-1 px-3 py-[13px]">
              <p class="t-hint">
                <Show when={r().matched.length} fallback="No rule matched.">
                  Matched: <For each={r().matched}>{(name, i) => <>{i() > 0 && ", "}{name}</>}</For>. Result goes to {r().output === "copy" ? "the clipboard" : "the cursor"}.
                </Show>
              </p>
              <pre class="t-control max-h-40 min-h-8 overflow-auto whitespace-pre-wrap break-words rounded-md bg-control px-2.5 py-1.5 select-text">{r().text}</pre>
            </div>
          )}
        </Show>
      </Card>
    </Section>
  );
}
