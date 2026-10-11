/**
 * Purpose: The form for one rule, laid out like a Raycast form: a back arrow and title, grouped cards, and a footer with Delete and Save.
 * Contents: RuleEditor — the whole form over a draft copy (Ctrl+Enter saves); Basics / Conditions / Actions / Result — its sections.
 */
import { Workflow, X } from "lucide-solid";
import { createSignal, For, onCleanup, onMount, Show, untrack } from "solid-js";
import { OUTPUT, TRIGGERS, WHEN, labelOf, newCondition, type ActionInfo, type Rule } from "../rules";
import { ConditionRow } from "./ConditionRow";
import { ActionHint, AddRow, Card, FooterBar, IconButton, Row, Section, Select, TextField, Toggle } from "./kit";
import { TestPanel } from "./TestPanel";

type Draft = { rule: () => Rule; patch: (part: Partial<Rule>) => void };

function Basics(p: Draft & { apps: () => string; setApps: (v: string) => void }) {
  return (
    <Section title="Rule">
      <Card>
        <Row label="Name"><div class="w-56"><TextField label="Name" value={p.rule().name} onInput={(name) => p.patch({ name })} /></div></Row>
        <Row label="Starts When"><Select label="Trigger" value={labelOf(TRIGGERS, p.rule().trigger)} options={Object.keys(TRIGGERS)} onChange={(l) => p.patch({ trigger: TRIGGERS[l] })} /></Row>
        <Row label="Only In" hint="Program names separated by commas. Empty means every program."><div class="w-56"><TextField label="Programs" placeholder="slack, chrome" value={p.apps()} onInput={p.setApps} /></div></Row>
      </Card>
    </Section>
  );
}

function Conditions(p: Draft) {
  const set = (i: number, c: Rule["conditions"][number]) => p.patch({ conditions: p.rule().conditions.map((x, at) => (at === i ? c : x)) });
  return (
    <Section title="Conditions">
      <Card>
        <Row label="Match" hint="With no conditions the rule applies to every text."><Select label="Match" value={labelOf(WHEN, p.rule().when)} options={Object.keys(WHEN)} onChange={(l) => p.patch({ when: WHEN[l] })} /></Row>
        <For each={p.rule().conditions}>
          {(c, i) => <ConditionRow value={c} onChange={(next) => set(i(), next)} onRemove={() => p.patch({ conditions: p.rule().conditions.filter((_, at) => at !== i()) })} />}
        </For>
        <AddRow label="Add Condition" onClick={() => p.patch({ conditions: [...p.rule().conditions, newCondition("contains")] })} />
      </Card>
    </Section>
  );
}

function Actions(p: Draft & { catalogue: ActionInfo[] }) {
  const label = (id: string) => p.catalogue.find((a) => a.id === id)?.label ?? id;
  const idOf = (l: string) => p.catalogue.find((a) => a.label === l)?.id ?? l;
  const set = (i: number, id: string) => p.patch({ actions: p.rule().actions.map((x, at) => (at === i ? id : x)) });
  return (
    <Section title="Actions">
      <Card>
        <For each={p.rule().actions}>
          {(id, i) => (
            <div class="flex items-center justify-between gap-2 px-3 py-1.5">
              <span class="t-hint w-6 shrink-0">{i() + 1}</span>
              <div class="min-w-0 flex-1"><Select field label="Action" value={label(id)} options={p.catalogue.map((a) => a.label)} onChange={(l) => set(i(), idOf(l))} /></div>
              <IconButton label="Remove action" onClick={() => p.patch({ actions: p.rule().actions.filter((_, at) => at !== i()) })}><X size={14} /></IconButton>
            </div>
          )}
        </For>
        <Show when={p.rule().actions.length === 0}><p class="t-hint px-3 py-[13px]">Add at least one action. Each one works on the result of the one before.</p></Show>
        <AddRow label="Add Action" onClick={() => p.patch({ actions: [...p.rule().actions, p.catalogue[0]?.id ?? ""] })} />
      </Card>
    </Section>
  );
}

function Result(p: Draft) {
  return (
    <Section title="Result">
      <Card>
        <Row label="Send To"><Select label="Send to" value={labelOf(OUTPUT, p.rule().output)} options={Object.keys(OUTPUT)} onChange={(l) => p.patch({ output: OUTPUT[l] })} /></Row>
        <Row label="Stop After This Rule" hint="Later rules do not run on a text this rule matched."><Toggle label="Stop after this rule" checked={p.rule().stop} onChange={(stop) => p.patch({ stop })} /></Row>
      </Card>
    </Section>
  );
}

const splitApps = (text: string) => text.split(",").map((a) => a.trim()).filter(Boolean);

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
  const keys = (e: KeyboardEvent) => e.ctrlKey && e.key === "Enter" && save();
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
      <TestPanel rules={() => p.rulesWith(full())} trigger={() => draft().trigger} />
      <Show when={p.error}><p class="t-hint px-3 !text-danger">{p.error}</p></Show>
      <FooterBar>
        <span class="t-label flex items-center gap-2"><Workflow size={16} class="text-muted" />{p.isNew ? "New Rule" : "Edit Rule"}</span>
        <div class="flex items-center gap-1">
          <Show when={p.onDelete}><ActionHint label="Delete Rule" danger onClick={() => p.onDelete?.()} /></Show>
          <ActionHint label="Save Rule" keys={["Ctrl", "↵"]} onClick={save} />
        </div>
      </FooterBar>
    </>
  );
}
