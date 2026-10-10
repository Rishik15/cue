/**
 * Purpose: One speech model on the Models page: name and badges, a one-line summary, accuracy and speed bars, then language, size and
 * the action that fits its state.
 * Contents: ModelCard — Download, a progress bar with Cancel, or Use and Delete (Delete asks once more).
 */
import { Globe, HardDrive, MemoryStick, Trash2 } from "lucide-solid";
import { createSignal, Show } from "solid-js";
import type { ModelInfo } from "../useModels";
import { Badge, Button, IconButton, Meter, Progress } from "./kit";

const mb = (bytes: number) => `${Math.round(bytes / 1_000_000)} MB`;
const gb = (megabytes: number) => `${(megabytes / 1000).toFixed(1)} GB`;

type Actions = { onUse: () => void; onDownload: () => void; onCancel: () => void; onDelete: () => void };

function Downloading(p: { percent: number; onCancel: () => void }) {
  return (
    <div class="flex items-center gap-2">
      <span class="t-meta w-9 text-right">{p.percent}%</span>
      <Progress value={p.percent} />
      <Button onClick={p.onCancel}>Cancel</Button>
    </div>
  );
}

/** Deleting a 650 MB download is not undone by a click: the first click asks, and it asks again if left alone for 3 seconds. */
function Delete(p: { onDelete: () => void }) {
  const [asking, setAsking] = createSignal(false);
  const ask = () => {
    setAsking(true);
    setTimeout(() => setAsking(false), 3000);
  };
  return (
    <Show when={asking()} fallback={<IconButton label="Delete" onClick={ask}><Trash2 size={14} /></IconButton>}>
      <Button danger onClick={p.onDelete}>Delete</Button>
    </Show>
  );
}

function Controls(p: { model: ModelInfo; selected: boolean; percent: number } & Actions) {
  return (
    <Show when={p.model.downloading} fallback={
      <Show when={p.model.installed} fallback={<Button onClick={p.onDownload}>Download</Button>}>
        <div class="flex items-center gap-2">
          <Delete onDelete={p.onDelete} />
          <Show when={!p.selected}><Button onClick={p.onUse}>Use</Button></Show>
        </div>
      </Show>
    }>
      <Downloading percent={p.percent} onCancel={p.onCancel} />
    </Show>
  );
}

export function ModelCard(p: { model: ModelInfo; selected: boolean; progress?: number } & Actions) {
  return (
    <div class="flex flex-col gap-3 px-3 py-[13px]">
      <div class="flex items-start justify-between gap-6">
        <div class="flex min-w-0 flex-col gap-0.5">
          <div class="flex items-center gap-2">
            <span class="t-label truncate" title={p.model.name}>{p.model.name}</span>
            <Show when={p.selected}><Badge accent>In Use</Badge></Show>
            <Show when={p.model.recommended && !p.selected}><Badge>Recommended</Badge></Show>
          </div>
          <div class="t-hint">{p.model.summary}</div>
        </div>
        <div class="flex shrink-0 flex-col gap-1">
          <Meter label="Accuracy" value={p.model.accuracy} />
          <Meter label="Speed" value={p.model.speed} />
        </div>
      </div>
      <div class="flex items-center justify-between gap-3">
        <div class="t-meta flex min-w-0 items-center gap-3">
          <span class="flex items-center gap-1.5"><Globe size={14} />{p.model.languages}</span>
          <span class="flex items-center gap-1.5" title="Download size"><HardDrive size={14} />{mb(p.model.size)}</span>
          <span class="flex items-center gap-1.5" title="Memory while dictating"><MemoryStick size={14} />{gb(p.model.memoryMb)}</span>
        </div>
        <Controls {...p} percent={p.progress ?? 0} />
      </div>
    </div>
  );
}
