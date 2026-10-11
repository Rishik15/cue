/**
 * Purpose: One condition of a rule: pick its type, fill its value, remove it.
 * Contents: ConditionRow — a type select, the value control that type needs, and a remove button.
 */
import { Minus } from "lucide-solid";
import { Match, Switch } from "solid-js";
import { CONDITION_TYPES, KINDS, labelOf, newCondition, type Condition } from "../rules";
import { IconButton, Select, TextField } from "./kit";

function Value(p: { value: Condition; onChange: (c: Condition) => void }) {
  const c = () => p.value;
  return (
    <Switch>
      <Match when={c().type === "looks_like" && (c() as Extract<Condition, { type: "looks_like" }>)}>
        {(l) => <Select field label="Kind" value={labelOf(KINDS, l().kind)} options={Object.keys(KINDS)} onChange={(k) => p.onChange({ type: "looks_like", kind: KINDS[k] })} />}
      </Match>
      <Match when={c().type === "longer_than" || c().type === "shorter_than"}>
        <TextField
          label="Characters"
          placeholder="Characters"
          value={String((c() as { chars: number }).chars)}
          onInput={(v) => p.onChange({ type: c().type as "longer_than", chars: Math.max(0, parseInt(v, 10) || 0) })}
        />
      </Match>
      <Match when={c().type === "matches"}>
        <TextField label="Pattern" placeholder="Pattern, for example ^\d+$" value={(c() as { pattern: string }).pattern} onInput={(v) => p.onChange({ type: "matches", pattern: v })} />
      </Match>
      <Match when={true}>
        <TextField label="Text" placeholder="Text" value={(c() as { value: string }).value} onInput={(v) => p.onChange({ type: c().type as "contains", value: v })} />
      </Match>
    </Switch>
  );
}

export function ConditionRow(p: { value: Condition; onChange: (c: Condition) => void; onRemove: () => void }) {
  return (
    <div class="flex items-center gap-2 px-3 py-1.5">
      <div class="w-36 shrink-0">
        <Select
          field
          label="Condition"
          value={labelOf(CONDITION_TYPES, p.value.type)}
          options={Object.keys(CONDITION_TYPES)}
          onChange={(l) => p.onChange(newCondition(CONDITION_TYPES[l]))}
        />
      </div>
      <Value value={p.value} onChange={p.onChange} />
      <IconButton label="Remove condition" onClick={p.onRemove}><Minus size={16} stroke-width={2} /></IconButton>
    </div>
  );
}
