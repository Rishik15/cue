/**
 * File: strip/ListView.tsx
 * Purpose: The Transform Text command list, loaded after the strip is on screen: the search line (the selection is not repeated: it is on screen beside the card), then every action under its group heading in the order the search ranks them, the highlighted one filled, then the footer with the highlighted action's hint and Run ↵. Assembled from `ui/Command`.
 * Contents: groupsOf — the hits as headed groups; ListView.
 */
import { createMemo, Show, untrack } from "solid-js";
import { ActionFooter } from "../ui/ActionFooter";
import { CommandInput, CommandList, type CommandGroup } from "../ui/Command";
import type { Flow } from "./flow";
import { GROUPS } from "./payload";

/** The hits as groups for the list: a new heading wherever the group changes (the search keeps a group's rows together). Input: the flow. Return: the groups, each row carrying its index in the hits. */
export function groupsOf(f: Flow): CommandGroup[] {
  const out: CommandGroup[] = [];
  f.hits.forEach((hit, index) => {
    const row = f.rows[hit.row];
    if (!row) return;
    const label = GROUPS[row.group] ?? row.group;
    let group = out[out.length - 1];
    if (group?.label !== label) out.push((group = { label, rows: [] }));
    group.rows.push({ index, name: row.name, hint: row.hint, spans: hit.spans });
  });
  return out;
}

/** The list view. The search line is always at the top, on either side of the text. Input: the flow and what typing, the pointer and a click do. */
export default function ListView(p: { flow: Flow; onType: (text: string) => void; onHover: (at: number) => void; onPick: (row: number) => void }) {
  /** The hits as headed groups, rebuilt only when the hits change (a new query), so moving the highlight keeps the rows and lets it glide. */
  const hits = createMemo(() => p.flow.hits);
  const groups = createMemo(() => {
    hits();
    return untrack(() => groupsOf(p.flow));
  });
  /** The highlighted action's hint, empty when nothing matches. */
  const hint = () => p.flow.rows[p.flow.hits[p.flow.selected]?.row ?? -1]?.hint ?? "";
  return (
    <>
      <CommandInput value={p.flow.query} onInput={(v) => p.onType(v)} active={p.flow.hits.length ? p.flow.selected : undefined} placeholder="Search actions" />
      <CommandList
        groups={groups()}
        selected={p.flow.selected}
        onHover={(at) => p.onHover(at)}
        onPick={(at) => p.onPick(p.flow.hits[at]?.row ?? -1)}
        empty={`No action matches "${p.flow.query}".`}
      />
      <Show when={hint()}>
        <ActionFooter title={<>{hint()}</>} primary={{ label: "Run", keys: ["↵"], onClick: () => p.onPick(p.flow.hits[p.flow.selected]?.row ?? -1) }} />
      </Show>
    </>
  );
}
