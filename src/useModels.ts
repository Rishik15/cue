/**
 * Purpose: State behind the Models pages: the catalog from Rust, live progress of every running download, and the actions.
 * Contents: useModels — signals plus download / cancel / remove; ModelInfo — one catalog row as Rust sends it. Listeners are registered before the first snapshot is read, and an event newer than the snapshot makes the page read it again (rules in `modelsLive`).
 * A refused command is shown as a toast; a download that fails is reported by Rust (`cue-toast`), which the toast also shows. Downloads run side by side, so a card is only ever blocked by its own state.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createSignal, onCleanup, onMount } from "solid-js";
import { behind, finished, fromSnapshot, progressed, type Job, type Live } from "./modelsLive";
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
  /** Needs more than a third of this PC's memory. */
  tight: boolean;
  /** Built for long sessions or for short phrases; empty for text models. */
  sessions: "long" | "short" | "";
  size: number;
  installed: boolean;
  /** The running download of this model, if any. */
  job: Job | null;
  /** Deleting now would clash with a running download. */
  deleteBlocked: boolean;
};
type Snapshot = { revision: number; models: ModelInfo[] };
type Progress = { id: string; generation: number; done: number; total: number };
type Finished = { id: string; generation: number; revision: number; outcome: string; error: string | null };
type Changed = { revision: number };

export function useModels() {
  const [models, setModels] = createSignal<ModelInfo[]>([]);
  const [live, setLive] = createSignal<Live>({});
  let revision = -1;
  let asked = 0;
  // Only the newest answer counts: a slow older one must not bring back a download that has ended since.
  const refresh = () => {
    const mine = ++asked;
    return invoke<Snapshot>("list_models").then((snap) => {
      if (mine !== asked) return;
      revision = snap.revision;
      setModels(snap.models);
      setLive(fromSnapshot(snap.models));
    }, () => showToast("Could not read the model list."));
  };
  const catchUp = (eventRevision: number) => { if (behind(revision, eventRevision)) void refresh(); };
  const act = (command: string, args: Record<string, unknown>) => invoke(command, args).then(refresh, (e) => showToast(String(e)));

  onMount(() => {
    const subs = [
      listen<Progress>("model-progress", (e) => setLive((l) => progressed(l, e.payload))),
      listen<Finished>("model-finished", (e) => {
        setLive((l) => finished(l, e.payload));
        setModels((all) => all.map((m) => (m.id === e.payload.id && (m.job?.generation ?? 0) <= e.payload.generation ? { ...m, job: null } : m))); // right away: a failed refresh must not leave the card stuck
        catchUp(e.payload.revision);
      }),
      listen<Changed>("model-changed", (e) => catchUp(e.payload.revision)),
    ];
    void Promise.all(subs).then(refresh);
    onCleanup(() => subs.forEach((s) => s.then((unlisten) => unlisten())));
  });

  return {
    models,
    progress: () => Object.fromEntries(Object.entries(live()).filter(([, j]) => !j.ended).map(([id, j]) => [id, j.percent])),
    download: (id: string) => act("download_model", { id }),
    cancel: (id: string) => { const job = live()[id]; if (job && !job.ended) void act("cancel_download", { id, generation: job.generation }); },
    remove: (id: string) => act("delete_model", { id }),
  };
}
