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

export function Rules() {
  const r = useRules();
  /** null: the list; -1: a new rule; otherwise the index being edited. */
  const [editing, setEditing] = createSignal<number | null>(null);
  const [start, setStart] = createSignal<Rule>(newRule());
  const open = (at: number, rule: Rule) => { setStart(rule); setEditing(at); };
  const rulesWith = (draft: Rule) => (editing() === -1 ? [...r.rules(), draft] : r.rules().map((x, i) => (i === editing() ? draft : x)));
  const done = (ok: boolean) => ok && setEditing(null);
  return (
    <Page>
      <Show when={editing() === null} fallback={
        <RuleEditor
          start={start()}
          actions={r.actions()}
          error={r.error()}
          rulesWith={rulesWith}
          onSave={(rule) => r.save(rulesWith(rule)).then(done)}
          onCancel={() => setEditing(null)}
          onDelete={editing() === -1 ? undefined : () => r.save(r.rules().filter((_, i) => i !== editing())).then(done)}
        />
      }>
        <Section title="Rules">
          <Card>
            <For each={r.rules()}>
              {(rule, i) => <RuleRow rule={rule} actions={r.actions()} onEdit={() => open(i(), rule)} onToggle={(enabled) => r.save(r.rules().map((x, at) => (at === i() ? { ...x, enabled } : x)))} />}
            </For>
            <Show when={r.rules().length === 0}><p class="t-hint px-3 py-[13px]">No rules yet. Start from a template or add your own.</p></Show>
            <div class="flex items-center justify-between gap-6 border-t border-line px-3 py-[13px] first:border-t-0">
              <span class="t-label">Add Rule</span>
              <IconButton label="Add rule" onClick={() => open(-1, newRule())}><Plus size={16} /></IconButton>
            </div>
          </Card>
          <Show when={r.error()}><p class="t-hint px-3 !text-danger">{r.error()}</p></Show>
        </Section>
        <RunLog />
        <Show when={r.rules().length === 0}>
          <Section title="Templates">
            <Card>
              <For each={TEMPLATES}>{(t) => (
                <div class="flex items-center justify-between gap-6 border-t border-line px-3 py-[13px] first:border-t-0">
                  <span class="t-label truncate" title={t.name}>{t.name}</span>
                  <Button onClick={() => open(-1, t)}>Use</Button>
                </div>
              )}</For>
            </Card>
          </Section>
        </Show>
      </Show>
    </Page>
  );
}
