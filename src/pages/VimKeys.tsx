/**
 * Purpose: The Vim key remapping card of the Notes page: one single-character field per action, the default as placeholder.
 * Contents: VimKeys — the card; a key already used by another action is refused in the field and never saved.
 */
import { createSignal, For } from "solid-js";
import { DEFAULT_KEYS } from "../notes/editor/vim/keys";
import { useSetting } from "../settings";
import { Button, Card, Row, TextField } from "../ui/kit";

const LABELS: Record<string, string> = {
  left: "Move left", down: "Move down", up: "Move up", right: "Move right", word: "Next word", back: "Previous word", end: "End of word",
  start: "Line start", finish: "Line end", insert: "Insert", append: "Append", below: "New line below", visual: "Visual",
  cut: "Delete character", paste: "Paste", undo: "Undo", help: "Help",
};

export function VimKeys(p: { disabled: boolean }) {
  const [keys, setKeys] = useSetting<Record<string, string>>("notes_vim_keys", {});
  const [refused, setRefused] = createSignal<{ action: string; key: string } | null>(null);
  const keyOf = (action: string) => keys()[action] ?? DEFAULT_KEYS[action];
  const taken = (action: string, key: string) => Object.keys(LABELS).some((a) => a !== action && keyOf(a) === key);

  const edit = (action: string, typed: string) => {
    const key = [...typed.trim()].pop() ?? "";
    const rest = Object.fromEntries(Object.entries(keys()).filter(([a]) => a !== action));
    if (key && taken(action, key)) return setRefused({ action, key });
    setRefused(null);
    setKeys(key ? { ...rest, [action]: key } : rest);
  };

  return (
    <div classList={{ "pointer-events-none opacity-40": p.disabled }} inert={p.disabled}>
      <Card>
        <For each={Object.entries(LABELS)}>{([action, label]) => (
          <Row id="notes-vim-keys" label={label}>
            <div class="w-14 [&_input]:text-center" classList={{ "[&_input]:!border-danger": refused()?.action === action }}>
              <TextField label={label} value={refused()?.action === action ? refused()!.key : keys()[action] ?? ""} placeholder={DEFAULT_KEYS[action]} onInput={(v) => edit(action, v)} />
            </div>
          </Row>
        )}</For>
        <Row label="Reset keys" hint="Back to the default keys"><Button disabled={Object.keys(keys()).length === 0} onClick={() => { setRefused(null); setKeys({}); }}>Reset</Button></Row>
      </Card>
    </div>
  );
}
