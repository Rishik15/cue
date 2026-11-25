/**
 * File: useModels.ts
 * Purpose: State behind the Models pages: the catalog from Rust, live progress of every running download, and the actions.
 * Contents: useModels — signals plus download / cancel / remove, wired to `connectModels` (rules in `modelsStore`); ModelInfo — re-exported for the cards.
 * A listener that cannot be registered marks the catalog unreadable and Retry subscribes again (one subscription at a time); once connected, Retry re-reads the snapshot.
 * A refused command is shown as a toast; a download that fails is reported by Rust (`cue-toast`), which the toast also shows. Downloads run side by side, so a card is only ever blocked by its own state.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal, onCleanup, onMount } from "solid-js";
import { connectModels, type Connection } from "./modelsIpc";
import { initial, progressOf, type State } from "./modelsStore";
import { showToast } from "./ui/Toast";

export type { ModelInfo } from "./modelsStore";

export function useModels() {
  const [state, setState] = createSignal<State>(initial);
  let link: Connection | undefined;
  let gone = false;
  let opening: Promise<void> | undefined;
  /** Opens the connection, one at a time (a second Retry while one is opening joins it); a refused subscription marks the catalog unreadable, so Retry connects again. */
  const connect = () =>
    (opening ??= connectModels(setState).then(
      (c) => { if (gone) c.stop(); else link = c; },
      () => { setState((s) => ({ ...s, unreadable: true })); },
    ).finally(() => { opening = undefined; }));
  onMount(() => void connect());
  onCleanup(() => { gone = true; link?.stop(); });
  const act = (command: string, args: Record<string, unknown>) => invoke(command, args).then(() => link?.refresh(), (e) => showToast(String(e)));
  return {
    models: () => state().models,
    unreadable: () => state().unreadable,
    retry: () => void (link ? link.refresh() : connect()),
    progress: () => progressOf(state()),
    download: (id: string) => act("download_model", { id }),
    cancel: (id: string) => { const job = state().live[id]; if (job) void act("cancel_download", { id, generation: job.generation }); },
    remove: (id: string) => act("delete_model", { id }),
  };
}
