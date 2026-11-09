/**
 * File: src/modelsIpc.ts
 * Purpose: The Tauri side of the Models page: listens to the download events, reads snapshots, validates every payload and feeds `modelsStore`.
 * Contents: connectModels — starts listening and returns the dispatcher, the stop function and the snapshot reader.
 * Listeners are registered before the first snapshot is read; whenever the store says it is behind, the snapshot is read again (one read at a time).
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { initial, needsSnapshot, parseChanged, parseFinished, parseProgress, parseSnapshot, reduce, type Msg, type State } from "./modelsStore";

export type Connection = { stop: () => void; refresh: () => Promise<void> };

/** Starts the connection; `onState` receives every new state. Resolves once the listeners are in place and the first snapshot was asked for. */
export async function connectModels(onState: (s: State) => void): Promise<Connection> {
  let state = initial;
  let reading = false;
  const dispatch = (m: Msg) => {
    state = reduce(state, m);
    onState(state);
    if (m.type !== "invalid" && needsSnapshot(state) && !reading) void refresh();
  };
  const refresh = async () => {
    reading = true;
    const snapshot = await invoke<unknown>("list_models").then(parseSnapshot, () => null);
    reading = false;
    dispatch(snapshot ? { type: "snapshot", snapshot } : { type: "invalid" });
  };
  const feed = <T>(parse: (v: unknown) => T | null, make: (v: T) => Msg) => (e: { payload: unknown }) => {
    const v = parse(e.payload);
    dispatch(v ? make(v) : { type: "invalid" });
  };
  const stops = await Promise.all([
    listen("model-progress", feed(parseProgress, (progress) => ({ type: "progress", progress }))),
    listen("model-finished", feed(parseFinished, (finished) => ({ type: "finished", finished }))),
    listen("model-changed", feed(parseChanged, (changed) => ({ type: "changed", changed }))),
  ]);
  await refresh();
  return { stop: () => stops.forEach((s) => s()), refresh };
}
