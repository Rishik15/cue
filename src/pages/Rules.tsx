/**
 * File: Rules.tsx
 * Purpose: The actions (stored as rules) of one feature (one trigger), shown inside that feature's page: the list (move one up or down, switch one on or off), recent runs, templates, and the
 * editor for adding or changing one, which replaces the whole page while open. Every feature page (Dictation, Text, later Files) hosts its own rules here.
 * Contents: Rules — list and editor over `useRules` for one trigger, with the page's other sections as `top`; RuleRow — one action in the list; Templates — the ready-made actions for the trigger.
 */
import { ChevronDown, ChevronUp } from "lucide-solid";
import { createEffect, createSignal, For, Index, type JSX, Show } from "solid-js";
import { closeDetail, current, openDetail } from "../nav";
import { moved, newRule, summary, type ActionInfo, type Rule, type Trigger } from "../rules";
import { TEMPLATES } from "../templates";
import { useRules } from "../useRules";
import { Button, Card, Hint, IconButton, ListRow, Page, PlusButton, Section, Toggle } from "../ui/kit";
import { RuleEditor } from "../ui/RuleEditor";
import { RunLog } from "../ui/RunLog";

const TITLE: Record<Trigger, string> = { dictation: "Inserted Text Actions", selection: "Transform Text Actions" };
const HINT: Record<Trigger, string> = {
  dictation: "Run on what you dictate before it is typed",
  selection: "Your own actions for selected text, shown above the built-in ones",
};

/** One saved rule as a list row. Props: the rule, the action catalog, whether it is first or last on its page, whether a save is queued (moves wait for it, since they name the rule by its place), and edit, enable/disable and move handlers. */
function RuleRow(p: { rule: Rule; actions: ActionInfo[]; first: boolean; last: boolean; busy: boolean; onEdit: () => void; onToggle: (on: boolean) => void; onMove: (by: -1 | 1) => void }) {
  return (
    <ListRow title={p.rule.name} subtitle={summary(p.rule, p.actions)} onClick={p.onEdit}>
      <div class="flex items-center gap-1.5">
        <IconButton label={`Move ${p.rule.name} up`} disabled={p.first || p.busy} onClick={() => p.onMove(-1)}><ChevronUp size={14} /></IconButton>
        <IconButton label={`Move ${p.rule.name} down`} disabled={p.last || p.busy} onClick={() => p.onMove(1)}><ChevronDown size={14} /></IconButton>
        <Toggle label={`${p.rule.name} enabled`} checked={p.rule.enabled} onChange={p.onToggle} />
      </div>
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
  busy: boolean;
  onEdit: (at: number, rule: Rule) => void;
  onToggle: (at: number, on: boolean) => void;
  onMove: (at: number, by: -1 | 1) => void;
};

/** The card opens with its title, a hint and a plus that adds a rule, each rule has move up and down buttons (order matters, ADR 0015) and a switch, and its row opens the editor, where it can be deleted. */
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
          {(e, i) => (
            <RuleRow rule={e().rule} actions={p.actions} first={i === 0} last={i === p.rules.length - 1} busy={p.busy}
              onEdit={() => p.onEdit(e().at, e().rule)} onToggle={(on) => p.onToggle(e().at, on)} onMove={(by) => p.onMove(e().at, by)} />
          )}
        </Index>
        <Show when={p.rules.length === 0}><Hint>Pick a template below</Hint></Show>
      </Card>
      <Show when={p.error}><p class="t-hint px-3 !text-danger">{p.error}</p></Show>
    </Section>
  );
}

/** The starter templates of a trigger, each with a Use button. Props: trigger, action catalog, handler given the chosen template. */
function Templates(p: { trigger: Trigger; actions: ActionInfo[]; onUse: (rule: Rule) => void }) {
  return (
    <Section title="Templates">
      <Card>
        <For each={TEMPLATES.filter((t) => t.trigger === p.trigger)}>{(t) => <ListRow title={t.name} subtitle={summary(t, p.actions)}><Button onClick={() => p.onUse(t)}>Use</Button></ListRow>}</For>
      </Card>
    </Section>
  );
}

/** The Rules page of a trigger: its saved rules, the editor and the templates. Props: the trigger and an optional top block. */
export function Rules(p: { trigger: Trigger; top?: JSX.Element }) {
  const r = useRules();
  /** null: the list; -1: a new rule; otherwise the index being edited. */
  const [editing, setEditing] = createSignal<number | null>(null);
  const [start, setStart] = createSignal<Rule>(newRule());
  /** This page's rules with their index in the full list. */
  const own = (): Entry[] => r.rules().flatMap((rule, at) => (rule.trigger === p.trigger ? [{ rule, at }] : []));
  /** Opens the editor on `rule` at index `at` (-1: new). */
  const open = (at: number, rule: Rule) => { setStart(rule); setEditing(at); openDetail(); };
  // The header's back arrow leaves the editor; the editor follows the history.
  createEffect(() => { if (!current().detail) setEditing(null); });
  /** `at` is the index being edited, or -1 for a new rule. */
  const place = (list: Rule[], draft: Rule, at: number | null) => (at === -1 ? [...list, draft] : list.map((x, i) => (i === at ? draft : x)));
  // A selection press runs one action, so its test runs the draft alone; dictation runs all of its actions in order.
  /** The rules a test runs: the draft alone for a selection, else the full list with the draft in place. */
  const rulesWith = (draft: Rule) => (p.trigger === "selection" ? [draft] : place(r.rules(), draft, editing()));
  /** Leaves the editor after a save or delete that worked. */
  const done = (ok: boolean) => ok && closeDetail();
  // A second click or Ctrl+Enter while the first save is still running would add a new rule twice.
  let saving = false;
  /** Saves the draft, once while a save is running. */
  const save = (rule: Rule) => {
    if (saving) return;
    saving = true;
    const at = editing();
    void r.update((list) => place(list, rule, at)).then(done).finally(() => { saving = false; });
  };
  /** Deletes the rule being edited. */
  const remove = () => { const at = editing(); return r.update((list) => list.filter((_, i) => i !== at)).then(done); };
  /** Turns the rule at `at` on or off. */
  const toggle = (at: number, enabled: boolean) => r.update((list) => list.map((x, i) => (i === at ? { ...x, enabled } : x)));
  /** Swaps the rule at `at` with the next one of this page up or down, past the other pages' rules. */
  const reorder = (at: number, by: -1 | 1) => r.update((list) => moved(list, at, by, (x) => x.trigger === p.trigger));
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
        <RuleList trigger={p.trigger} rules={own()} actions={r.actions()} error={r.error()} busy={r.busy()} onEdit={open} onToggle={toggle} onMove={reorder} />
        <RunLog trigger={p.trigger} />
        <Templates trigger={p.trigger} actions={r.actions()} onUse={(t) => open(-1, t)} />
      </Show>
    </Page>
  );
}
