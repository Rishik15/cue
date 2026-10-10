/**
 * Purpose: State behind the Models page: the catalog from Rust, live download progress, and the actions.
 * Contents: useModels — signals plus download / cancel / remove; ModelInfo — one catalog row as Rust sends it.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createSignal, onCleanup, onMount } from "solid-js";

export type ModelInfo = {
  id: string;
  name: string;
  summary: string;
  languages: string;
  accuracy: number;
  speed: number;
  memoryMb: number;
  recommended: boolean;
  size: number;
  installed: boolean;
  downloading: boolean;
};
type Progress = { id: string; done: number; total: number };
type Finished = { id: string; error: string | null };

export function useModels() {
  const [models, setModels] = createSignal<ModelInfo[]>([]);
  const [progress, setProgress] = createSignal<Record<string, number>>({});
  const [error, setError] = createSignal("");
  const refresh = () => invoke<ModelInfo[]>("list_models").then(setModels, () => setError("Could not read the model list."));
  const act = (command: string, id: string) => invoke(command, { id }).then(refresh, (e) => setError(String(e)));

  onMount(() => {
    refresh();
    const subs = [
      listen<Progress>("model-progress", (e) => setProgress((p) => ({ ...p, [e.payload.id]: Math.floor((100 * e.payload.done) / e.payload.total) }))),
      listen<Finished>("model-finished", (e) => {
        setProgress((p) => Object.fromEntries(Object.entries(p).filter(([id]) => id !== e.payload.id)));
        setError(e.payload.error ?? "");
        refresh();
      }),
    ];
    onCleanup(() => subs.forEach((s) => s.then((unlisten) => unlisten())));
  });

  return {
    models,
    progress,
    error,
    download: (id: string) => { setError(""); setProgress((p) => ({ ...p, [id]: 0 })); act("download_model", id); },
    cancel: (id: string) => act("cancel_download", id),
    remove: (id: string) => act("delete_model", id),
  };
}
