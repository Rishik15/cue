/**
 * Purpose: Rules page: the list of rules (switch one on or off), and the editor for adding or changing one.
 * Contents: Rules — list and editor over `useRules`; RuleRow — one rule in the list; TEMPLATES — starting points for an empty list.
 */
import { createEffect, createSignal, For, Index, Show } from "solid-js";
import { closeDetail, current, openDetail } from "../nav";
import { newRule, summary, type ActionInfo, type Rule } from "../rules";
import { useRules } from "../useRules";
import { Button, Card, ListRow, Page, PlusButton, Section, Toggle } from "../ui/kit";
import { RuleEditor } from "../ui/RuleEditor";
import { RunLog } from "../ui/RunLog";

const TEMPLATES: Rule[] = [
  { ...newRule(), name: "Tidy Dictation Spacing", actions: ["clean_whitespace"] },
  { ...newRule(), name: "Clean Copied Links", trigger: "selection", conditions: [{ type: "contains", value: "http" }], actions: ["clean_links"] },
];

function RuleRow(p: { rule: Rule; actions: ActionInfo[]; onEdit: () => void; onToggle: (on: boolean) => void }) {
  return (
    <ListRow title={p.rule.name} subtitle={summary(p.rule, p.actions)} onClick={p.onEdit}>
      <Toggle label={`${p.rule.name} enabled`} checked={p.rule.enabled} onChange={p.onToggle} />
    </ListRow>
  );
}

type ListProps = {
  rules: Rule[];
  actions: ActionInfo[];
  error: string;
  onEdit: (at: number, rule: Rule) => void;
  onToggle: (at: number, on: boolean) => void;
};

/** The card opens with its title, a hint and a plus that adds a rule, each rule has a switch, and its row opens the editor, where it can be deleted. */
function RuleList(p: ListProps) {
  return (
    <Section id="rules-list">
      <Card>
        <div class="flex items-start justify-between gap-6 px-3 py-[13px]">
          <div class="flex min-w-0 flex-col gap-0.5">
            <div class="t-label">Rules</div>
            <div class="t-hint max-w-[52ch]">Change text automatically when you finish dictating or use the selection shortcut.</div>
          </div>
          <PlusButton label="Add rule" onClick={() => p.onEdit(-1, newRule())} />
        </div>
        {/* Index, not For: a saved toggle replaces the rule object, and For would rebuild the row and skip the switch's slide. */}
        <Index each={p.rules}>
          {(rule, i) => <RuleRow rule={rule()} actions={p.actions} onEdit={() => p.onEdit(i, rule())} onToggle={(on) => p.onToggle(i, on)} />}
        </Index>
        <Show when={p.rules.length === 0}><p class="t-hint px-3 py-[13px]">No rules yet. Start from a template or add your own.</p></Show>
      </Card>
      <Show when={p.error}><p class="t-hint px-3 !text-danger">{p.error}</p></Show>
    </Section>
  );
}

function Templates(p: { actions: ActionInfo[]; onUse: (rule: Rule) => void }) {
  return (
    <Section title="Templates">
      <Card>
        <For each={TEMPLATES}>{(t) => <ListRow title={t.name} subtitle={summary(t, p.actions)}><Button onClick={() => p.onUse(t)}>Use</Button></ListRow>}</For>
      </Card>
    </Section>
  );
}

export function Rules() {
  const r = useRules();
  /** null: the list; -1: a new rule; otherwise the index being edited. */
  const [editing, setEditing] = createSignal<number | null>(null);
  const [start, setStart] = createSignal<Rule>(newRule());
  const open = (at: number, rule: Rule) => { setStart(rule); setEditing(at); openDetail(); };
  // The header's back arrow leaves the editor; the editor follows the history.
  createEffect(() => { if (!current().detail) setEditing(null); });
  /** `at` is the index being edited, or -1 for a new rule. */
  const place = (list: Rule[], draft: Rule, at: number | null) => (at === -1 ? [...list, draft] : list.map((x, i) => (i === at ? draft : x)));
  const rulesWith = (draft: Rule) => place(r.rules(), draft, editing());
  const done = (ok: boolean) => ok && closeDetail();
  const save = (rule: Rule) => { const at = editing(); return r.update((list) => place(list, rule, at)).then(done); };
  const remove = () => { const at = editing(); return r.update((list) => list.filter((_, i) => i !== at)).then(done); };
  const toggle = (at: number, enabled: boolean) => r.update((list) => list.map((x, i) => (i === at ? { ...x, enabled } : x)));
  return (
    <Page>
      <Show when={editing() === null} fallback={
        <RuleEditor
          start={start()}
          isNew={editing() === -1}
          actions={r.actions()}
          error={r.error()}
          rulesWith={rulesWith}
          onSave={(rule) => save(rule)}
          onDelete={editing() === -1 ? undefined : () => remove()}
        />
      }>
        <RuleList rules={r.rules()} actions={r.actions()} error={r.error()} onEdit={open} onToggle={toggle} />
        <RunLog />
        <Show when={r.rules().length === 0}><Templates actions={r.actions()} onUse={(t) => open(-1, t)} /></Show>
      </Show>
    </Page>
  );
}
