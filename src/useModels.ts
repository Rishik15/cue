/**
 * Purpose: State behind the Models page: the catalog from Rust, live download progress, and the actions.
 * Contents: useModels — signals plus download / cancel / remove; ModelInfo — one catalog row as Rust sends it. A refused command is shown as a toast; a download that fails is reported by Rust (`cue-status`), which the toast also shows.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createSignal, onCleanup, onMount } from "solid-js";
import { showToast } from "./ui/Toast";

export type ModelInfo = {
  id: string;
  kind: "speech" | "text" | "dictionary";
  name: string;
  summary: string;
  languages: string;
  accuracy: number;
  speed: number;
  memoryMb: number;
  recommended: boolean;
  /** Built for long sessions or for short phrases; empty for text models. */
  sessions: "long" | "short" | "";
  size: number;
  installed: boolean;
  downloading: boolean;
};
type Progress = { id: string; done: number; total: number };
type Finished = { id: string; error: string | null };

export function useModels() {
  const [models, setModels] = createSignal<ModelInfo[]>([]);
  const [progress, setProgress] = createSignal<Record<string, number>>({});
  const refresh = () => invoke<ModelInfo[]>("list_models").then(setModels, () => showToast("Could not read the model list."));
  const clearProgress = (id: string) => setProgress((p) => Object.fromEntries(Object.entries(p).filter(([key]) => key !== id)));
  // A refused command (another download running, say) must not leave the card stuck at 0 %.
  const act = (command: string, id: string) => invoke(command, { id }).then(refresh, (e) => { clearProgress(id); showToast(String(e)); });

  onMount(() => {
    void refresh();
    const subs = [
      listen<Progress>("model-progress", (e) => setProgress((p) => ({ ...p, [e.payload.id]: Math.floor((100 * e.payload.done) / e.payload.total) }))),
      listen<Finished>("model-finished", (e) => {
        clearProgress(e.payload.id);
        void refresh();
      }),
    ];
    onCleanup(() => subs.forEach((s) => s.then((unlisten) => unlisten())));
  });

  return {
    models,
    progress,
    download: (id: string) => { setProgress((p) => ({ ...p, [id]: 0 })); void act("download_model", id); },
    cancel: (id: string) => act("cancel_download", id),
    remove: (id: string) => act("delete_model", id),
  };
}
