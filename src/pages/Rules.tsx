/**
 * Purpose: The actions (stored as rules) of one feature (one trigger), shown inside that feature's page: the list (switch one on or off), recent runs, templates, and the
 * editor for adding or changing one, which replaces the whole page while open. Every feature page (Dictation, Text, later Files) hosts its own rules here.
 * Contents: Rules — list and editor over `useRules` for one trigger, with the page's other sections as `top`; RuleRow — one action in the list; Templates — the ready-made actions for the trigger.
 */
import { createEffect, createSignal, For, Index, type JSX, Show } from "solid-js";
import { closeDetail, current, openDetail } from "../nav";
import { newRule, summary, type ActionInfo, type Rule, type Trigger } from "../rules";
import { TEMPLATES } from "../templates";
import { useRules } from "../useRules";
import { Button, Card, ListRow, Page, PlusButton, Section, Toggle } from "../ui/kit";
import { RuleEditor } from "../ui/RuleEditor";
import { RunLog } from "../ui/RunLog";

const TITLE: Record<Trigger, string> = { dictation: "Inserted Text Actions", selection: "Actions" };
const HINT: Record<Trigger, string> = {
  dictation: "Run on dictated text before it is inserted",
  selection: "Your actions for selected text, listed before the built-in ones",
};

function RuleRow(p: { rule: Rule; actions: ActionInfo[]; onEdit: () => void; onToggle: (on: boolean) => void }) {
  return (
    <ListRow title={p.rule.name} subtitle={summary(p.rule, p.actions)} onClick={p.onEdit}>
      <Toggle label={`${p.rule.name} enabled`} checked={p.rule.enabled} onChange={p.onToggle} />
    </ListRow>
  );
}

/** A rule with its place in the full saved list, since a page shows only its own trigger's rules. */
type Entry = { rule: Rule; at: number };

type ListProps = {
  trigger: Trigger;
  rules: Entry[];
  actions: ActionInfo[];
  error: string;
  onEdit: (at: number, rule: Rule) => void;
  onToggle: (at: number, on: boolean) => void;
};

/** The card opens with its title, a hint and a plus that adds a rule, each rule has a switch, and its row opens the editor, where it can be deleted. */
function RuleList(p: ListProps) {
  return (
    <Section id={p.trigger === "dictation" ? "dictation-rules" : "text-rules"}>
      <Card>
        <div class="flex items-start justify-between gap-6 px-3 py-[13px]">
          <div class="flex min-w-0 flex-col gap-0.5">
            <div class="t-label">{TITLE[p.trigger]}</div>
            <div class="t-hint max-w-[52ch]">{HINT[p.trigger]}</div>
          </div>
          <PlusButton label="Add action" onClick={() => p.onEdit(-1, { ...newRule(), trigger: p.trigger })} />
        </div>
        {/* Index, not For: a saved toggle replaces the rule object, and For would rebuild the row and skip the switch's slide. */}
        <Index each={p.rules}>
          {(e) => <RuleRow rule={e().rule} actions={p.actions} onEdit={() => p.onEdit(e().at, e().rule)} onToggle={(on) => p.onToggle(e().at, on)} />}
        </Index>
        <Show when={p.rules.length === 0}><p class="t-hint px-3 py-[13px]">No actions yet</p></Show>
      </Card>
      <Show when={p.error}><p class="t-hint px-3 !text-danger">{p.error}</p></Show>
    </Section>
  );
}

function Templates(p: { trigger: Trigger; actions: ActionInfo[]; onUse: (rule: Rule) => void }) {
  return (
    <Section title="Templates">
      <Card>
        <For each={TEMPLATES.filter((t) => t.trigger === p.trigger)}>{(t) => <ListRow title={t.name} subtitle={summary(t, p.actions)}><Button onClick={() => p.onUse(t)}>Use</Button></ListRow>}</For>
      </Card>
    </Section>
  );
}

export function Rules(p: { trigger: Trigger; top?: JSX.Element }) {
  const r = useRules();
  /** null: the list; -1: a new rule; otherwise the index being edited. */
  const [editing, setEditing] = createSignal<number | null>(null);
  const [start, setStart] = createSignal<Rule>(newRule());
  const own = (): Entry[] => r.rules().flatMap((rule, at) => (rule.trigger === p.trigger ? [{ rule, at }] : []));
  const open = (at: number, rule: Rule) => { setStart(rule); setEditing(at); openDetail(); };
  // The header's back arrow leaves the editor; the editor follows the history.
  createEffect(() => { if (!current().detail) setEditing(null); });
  /** `at` is the index being edited, or -1 for a new rule. */
  const place = (list: Rule[], draft: Rule, at: number | null) => (at === -1 ? [...list, draft] : list.map((x, i) => (i === at ? draft : x)));
  // A selection press runs one action, so its test runs the draft alone; dictation runs all of its actions in order.
  const rulesWith = (draft: Rule) => (p.trigger === "selection" ? [draft] : place(r.rules(), draft, editing()));
  const done = (ok: boolean) => ok && closeDetail();
  // A second click or Ctrl+Enter while the first save is still running would add a new rule twice.
  let saving = false;
  const save = (rule: Rule) => {
    if (saving) return;
    saving = true;
    const at = editing();
    void r.update((list) => place(list, rule, at)).then(done).finally(() => { saving = false; });
  };
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
          onSave={save}
          onDelete={editing() === -1 ? undefined : () => remove()}
        />
      }>
        {p.top}
        <RuleList trigger={p.trigger} rules={own()} actions={r.actions()} error={r.error()} onEdit={open} onToggle={toggle} />
        <RunLog trigger={p.trigger} />
        <Templates trigger={p.trigger} actions={r.actions()} onUse={(t) => open(-1, t)} />
      </Show>
    </Page>
  );
}
