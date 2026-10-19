/**
 * Purpose: Picks the folder of a snippet from the folders that exist, or makes a new one on the spot without leaving the form.
 * Contents: FolderPicker — two rows of a form card: a select of "No Folder", the folders and "New Folder…", and (after that choice) a row with a name field and an Add button.
 */
import { createSignal, Show } from "solid-js";
import type { Folder } from "../snippets";
import { Button, Row, Select, TextField } from "./kit";

const NONE = "No Folder";
const NEW = "New Folder…";

export function FolderPicker(p: { folders: Folder[]; value: number | null; onChange: (id: number | null) => void; onCreate: (name: string) => Promise<number | null> }) {
  const [naming, setNaming] = createSignal(false);
  const [name, setName] = createSignal("");
  // Options are matched by position, so a folder named like a fixed option cannot be mistaken for it.
  const shown = (n: string) => (n === NONE || n === NEW ? `“${n}”` : n);
  const labels = () => [NONE, ...p.folders.map((f) => shown(f.name)), NEW];
  const label = () => {
    const at = p.folders.findIndex((f) => f.id === p.value);
    return at < 0 ? NONE : labels()[at + 1] ?? NONE;
  };
  const choose = (l: string) => {
    const at = labels().indexOf(l);
    const isNew = at === labels().length - 1;
    setNaming(isNew);
    if (!isNew) p.onChange(at > 0 ? p.folders[at - 1]?.id ?? null : null);
  };
  const add = async () => {
    const typed = name().trim();
    if (!typed) return;
    const same = p.folders.find((f) => f.name.toLowerCase() === typed.toLowerCase());
    const id = same ? same.id : await p.onCreate(typed);
    if (id === null) return;
    setNaming(false);
    setName("");
    p.onChange(id);
  };
  return (
    <>
      <Row label="Folder"><div class="flex w-56 justify-end"><Select label="Folder" value={naming() ? NEW : label()} options={labels()} onChange={choose} /></div></Row>
      <Show when={naming()}>
        <Row label="New Folder">
          <div class="flex w-56 items-center gap-2" onKeyDown={(e) => { if (e.key === "Enter" && !e.ctrlKey) { e.preventDefault(); void add(); } }}>
            <TextField label="New folder name" placeholder="Folder name" value={name()} onInput={setName} autofocus />
            <Button onClick={() => void add()}>Add</Button>
          </div>
        </Row>
      </Show>
    </>
  );
}
