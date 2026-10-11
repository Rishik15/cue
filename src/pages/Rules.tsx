/**
 * Purpose: Rules page: the list of rules (switch one on or off), and the editor for adding or changing one.
 * Contents: Rules — list and editor over `useRules`; RuleRow — one rule in the list; TEMPLATES — starting points for an empty list.
 */
import { Plus } from "lucide-solid";
import { createSignal, For, Show } from "solid-js";
import { newRule, summary, type ActionInfo, type Rule } from "../rules";
import { useRules } from "../useRules";
import { Button, Card, IconButton, Page, Section, Toggle } from "../ui/kit";
import { RuleEditor } from "../ui/RuleEditor";
import { RunLog } from "../ui/RunLog";

const TEMPLATES: Rule[] = [
  { ...newRule(), name: "Tidy Dictation Spacing", actions: ["clean_whitespace"] },
  { ...newRule(), name: "Clean Copied Links", trigger: "selection", conditions: [{ type: "contains", value: "http" }], actions: ["clean_links"] },
];

function RuleRow(p: { rule: Rule; actions: ActionInfo[]; onEdit: () => void; onToggle: (on: boolean) => void }) {
  return (
    <div class="flex items-center gap-3 border-t border-line px-3 py-[13px] first:border-t-0">
      <button onClick={() => p.onEdit()} class="flex min-w-0 flex-1 flex-col gap-0.5 text-left" title={p.rule.name}>
        <span class="t-label truncate">{p.rule.name}</span>
        <span class="t-hint truncate">{summary(p.rule, p.actions)}</span>
      </button>
      <Toggle label={`${p.rule.name} enabled`} checked={p.rule.enabled} onChange={p.onToggle} />
    </div>
  );
}

function RuleList(p: { rules: Rule[]; actions: ActionInfo[]; error: string; onEdit: (at: number, rule: Rule) => void; onToggle: (at: number, on: boolean) => void }) {
  return (
    <Section title="Rules">
      <Card>
        <For each={p.rules}>
          {(rule, i) => <RuleRow rule={rule} actions={p.actions} onEdit={() => p.onEdit(i(), rule)} onToggle={(on) => p.onToggle(i(), on)} />}
        </For>
        <Show when={p.rules.length === 0}><p class="t-hint px-3 py-[13px]">No rules yet. Start from a template or add your own.</p></Show>
        <div class="flex items-center justify-between gap-6 border-t border-line px-3 py-[13px] first:border-t-0">
          <span class="t-label">Add Rule</span>
          <IconButton label="Add rule" onClick={() => p.onEdit(-1, newRule())}><Plus size={16} /></IconButton>
        </div>
      </Card>
      <Show when={p.error}><p class="t-hint px-3 !text-danger">{p.error}</p></Show>
    </Section>
  );
}

function Templates(p: { onUse: (rule: Rule) => void }) {
  return (
    <Section title="Templates">
      <Card>
        <For each={TEMPLATES}>{(t) => (
          <div class="flex items-center justify-between gap-6 border-t border-line px-3 py-[13px] first:border-t-0">
            <span class="t-label truncate" title={t.name}>{t.name}</span>
            <Button onClick={() => p.onUse(t)}>Use</Button>
          </div>
        )}</For>
      </Card>
    </Section>
  );
}

export function Rules() {
  const r = useRules();
  /** null: the list; -1: a new rule; otherwise the index being edited. */
  const [editing, setEditing] = createSignal<number | null>(null);
  const [start, setStart] = createSignal<Rule>(newRule());
  const open = (at: number, rule: Rule) => { setStart(rule); setEditing(at); };
  /** `at` is the index being edited, or -1 for a new rule. */
  const place = (list: Rule[], draft: Rule, at: number | null) => (at === -1 ? [...list, draft] : list.map((x, i) => (i === at ? draft : x)));
  const rulesWith = (draft: Rule) => place(r.rules(), draft, editing());
  const done = (ok: boolean) => ok && setEditing(null);
  const save = (rule: Rule) => { const at = editing(); return r.update((list) => place(list, rule, at)).then(done); };
  const remove = () => { const at = editing(); return r.update((list) => list.filter((_, i) => i !== at)).then(done); };
  const toggle = (at: number, enabled: boolean) => r.update((list) => list.map((x, i) => (i === at ? { ...x, enabled } : x)));
  return (
    <Page>
      <Show when={editing() === null} fallback={
        <RuleEditor
          start={start()}
          actions={r.actions()}
          error={r.error()}
          rulesWith={rulesWith}
          onSave={(rule) => save(rule)}
          onCancel={() => setEditing(null)}
          onDelete={editing() === -1 ? undefined : () => remove()}
        />
      }>
        <RuleList rules={r.rules()} actions={r.actions()} error={r.error()} onEdit={open} onToggle={toggle} />
        <RunLog />
        <Show when={r.rules().length === 0}><Templates onUse={(t) => open(-1, t)} /></Show>
      </Show>
    </Page>
  );
}
