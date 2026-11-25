/**
 * File: ui/RuleEditor.tsx
 * Purpose: The form for one rule: grouped cards, a live test, then a Save button (and a minus that deletes an existing rule).
 * Contents: RuleEditor — the whole form over a draft copy (Ctrl+Enter saves); Basics / Conditions / Actions / Result — its sections.
 */
import { createEffect, createSignal, For, onCleanup, onMount, Show, untrack } from "solid-js";
import { setDetailTitle } from "../nav";
import { OUTPUT, WHEN, labelOf, newCondition, type ActionInfo, type Rule } from "../rules";
import { ConditionRow } from "./ConditionRow";
import { ConfirmButton } from "./ConfirmButton";
import { composing } from "./keys";
import { AddRow, Button, Card, Hint, MinusButton, Row, Section, Select, TextField, Toggle } from "./kit";
import { sampleFor } from "../templates";
import { TestPanel } from "./TestPanel";

type Draft = { rule: () => Rule; patch: (part: Partial<Rule>) => void };

/** The name and programs card. Input: the draft and the programs text. Return: the section. */
function Basics(p: Draft & { apps: () => string; setApps: (v: string) => void }) {
  return (
    <Section title="Action">
      <Card>
        <Row label="Name"><div class="w-56"><TextField label="Name" value={p.rule().name} onInput={(name) => p.patch({ name })} /></div></Row>
        <Row label="Only In Programs" hint="App names separated by commas. Leave empty for all apps"><div class="w-56"><TextField label="Programs" placeholder="slack, chrome" value={p.apps()} onInput={p.setApps} /></div></Row>
      </Card>
    </Section>
  );
}

/** The conditions card: each condition and a row that adds one. Input: the draft. Return: the section. */
function Conditions(p: Draft) {
  const set = (i: number, c: Rule["conditions"][number]) => p.patch({ conditions: p.rule().conditions.map((x, at) => (at === i ? c : x)) });
  return (
    <Section title="Conditions">
      <Card>
        <Row label="Apply When" hint="With no conditions, this runs on all text"><Select label="Apply when" value={labelOf(WHEN, p.rule().when)} options={Object.keys(WHEN)} onChange={(l) => p.patch({ when: WHEN[l] })} /></Row>
        <For each={p.rule().conditions}>
          {(c, i) => <ConditionRow value={c} onChange={(next) => set(i(), next)} onRemove={() => p.patch({ conditions: p.rule().conditions.filter((_, at) => at !== i()) })} />}
        </For>
        <AddRow label="Add Condition" onClick={() => p.patch({ conditions: [...p.rule().conditions, newCondition("contains")] })} />
      </Card>
    </Section>
  );
}

/** The steps card: each step as a select and a row that adds one. Input: the draft and the action catalogue. Return: the section. */
function Actions(p: Draft & { catalogue: ActionInfo[] }) {
  /** Dictation offers the steps that make sense on speech; selected text takes all of them. */
  const offered = () => (p.rule().trigger === "dictation" ? p.catalogue.filter((a) => a.dictation) : p.catalogue);
  const label = (id: string) => p.catalogue.find((a) => a.id === id)?.label ?? id;
  const idOf = (l: string) => p.catalogue.find((a) => a.label === l)?.id ?? l;
  const set = (i: number, id: string) => p.patch({ actions: p.rule().actions.map((x, at) => (at === i ? id : x)) });
  return (
    <Section title="Steps">
      <Card>
        <For each={p.rule().actions}>
          {(id, i) => (
            <div class="flex items-center justify-between gap-2 px-3 py-1.5">
              <span class="t-hint w-6 shrink-0">{i() + 1}</span>
              <div class="min-w-0 flex-1"><Select field label="Step" value={label(id)} options={offered().map((a) => a.label)} onChange={(l) => set(i(), idOf(l))} /></div>
              <MinusButton label="Remove step" onClick={() => p.patch({ actions: p.rule().actions.filter((_, at) => at !== i()) })} />
            </div>
          )}
        </For>
        <Show when={p.rule().actions.length === 0}><Hint>No steps. Each step works on the previous result.</Hint></Show>
        <AddRow label="Add Step" onClick={() => p.patch({ actions: [...p.rule().actions, offered()[0]?.id ?? ""] })} />
      </Card>
    </Section>
  );
}

/** Where the result goes. Input: the draft. Return: the section. */
function Result(p: Draft) {
  return (
    <Section title="Result">
      <Card>
        <Row label="Send Result To"><Select label="Send result to" value={labelOf(OUTPUT, p.rule().output)} options={Object.keys(OUTPUT)} onChange={(l) => p.patch({ output: OUTPUT[l] })} /></Row>
        <Show when={p.rule().trigger === "dictation"}><Row label="Stop After This Action" hint="Don't run later actions once this one matches"><Toggle label="Stop after this action" checked={p.rule().stop} onChange={(stop) => p.patch({ stop })} /></Row></Show>
      </Card>
    </Section>
  );
}

/** The programs text as program names. Input: comma-separated names. Return: the non-empty names. */
const splitApps = (text: string) => text.split(",").map((a) => a.trim()).filter(Boolean);

/** The rule form over a draft copy; Ctrl+Enter saves (never during an input-method composition). Inputs: the rule, whether it is new, the catalogue and the save and delete handlers. Return: the form. */
export function RuleEditor(p: {
  start: Rule;
  isNew: boolean;
  actions: ActionInfo[];
  error: string;
  rulesWith: (draft: Rule) => Rule[];
  onSave: (rule: Rule) => void;
  onDelete?: () => void;
}) {
  // The form edits its own copy of the rule it was opened with.
  const [draft, setDraft] = createSignal(untrack(() => p.start));
  const [apps, setApps] = createSignal(untrack(() => p.start.apps.join(", ")));
  const patch = (part: Partial<Rule>) => setDraft({ ...draft(), ...part });
  const full = () => ({ ...draft(), apps: splitApps(apps()) });
  const save = () => p.onSave(full());
  createEffect(() => setDetailTitle(p.isNew ? "New Action" : "Edit Action"));
  /** Ctrl+Enter saves, except while an input method composes. Input: the key event. */
  const keys = (e: KeyboardEvent) => !composing(e) && e.ctrlKey && e.key === "Enter" && save();
  onMount(() => {
    window.addEventListener("keydown", keys);
    onCleanup(() => window.removeEventListener("keydown", keys));
  });
  return (
    <>
      <Basics rule={draft} patch={patch} apps={apps} setApps={setApps} />
      <Conditions rule={draft} patch={patch} />
      <Actions rule={draft} patch={patch} catalogue={p.actions} />
      <Result rule={draft} patch={patch} />
      <TestPanel rules={() => p.rulesWith(full())} trigger={() => draft().trigger} initial={untrack(() => sampleFor(p.start))} actions={p.actions} />
      <Show when={p.error}><p class="t-hint px-3 !text-danger">{p.error}</p></Show>
      <div class="flex items-center justify-between">
        <Show when={p.onDelete} fallback={<span />}><ConfirmButton onConfirm={() => p.onDelete?.()} idle={(arm) => <Button danger onClick={arm}>Delete Action</Button>} /></Show>
        <Button onClick={save}>Save Action</Button>
      </div>
    </>
  );
}
