/**
 * Purpose: Picks the folder of a snippet from the folders that exist, or makes a new one on the spot without leaving the form.
 * Contents: FolderPicker — a select of "No Folder", the folders and "New Folder…", which opens a name field and an Add button.
 */
import { createSignal, Show } from "solid-js";
import type { Folder } from "../snippets";
import { Button, Select, TextField } from "./kit";

const NONE = "No Folder";
const NEW = "New Folder…";

export function FolderPicker(p: { folders: Folder[]; value: number | null; onChange: (id: number | null) => void; onCreate: (name: string) => Promise<number | null> }) {
  const [naming, setNaming] = createSignal(false);
  const [name, setName] = createSignal("");
  const label = () => p.folders.find((f) => f.id === p.value)?.name ?? NONE;
  const choose = (l: string) => {
    setNaming(l === NEW);
    if (l !== NEW) p.onChange(p.folders.find((f) => f.name === l)?.id ?? null);
  };
  const add = async () => {
    const id = await p.onCreate(name().trim());
    if (id === null) return;
    setNaming(false);
    setName("");
    p.onChange(id);
  };
  return (
    <div class="flex w-56 flex-col items-end gap-2">
      <Select label="Folder" value={naming() ? NEW : label()} options={[NONE, ...p.folders.map((f) => f.name), NEW]} onChange={choose} />
      <Show when={naming()}>
        <div class="flex w-full items-center gap-2">
          <TextField label="New folder name" placeholder="Folder name" value={name()} onInput={setName} autofocus />
          <Button onClick={() => void add()}>Add</Button>
        </div>
      </Show>
    </div>
  );
}
