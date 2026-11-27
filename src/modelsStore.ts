/**
 * File: src/modelsStore.ts
 * Purpose: The one place that decides what the Models page shows while downloads start, move and end from several windows: a reducer over snapshots and events, plus the guards that reject anything Rust did not mean to send.
 * Contents: ModelInfo / Job / Last / Snapshot — the data; lastResult — the card's line about the previous download; parseSnapshot / parseProgress / parseFinished / parseChanged — IPC guards returning null for a bad shape; reduce / initial / needsSnapshot / progressOf — the state machine and its reads.
 * Checks are built from `src/ipc.ts`.
 * Rules: a snapshot older than the one held is ignored; progress that arrives before the first snapshot is kept and replayed after it; a finished generation of a model is remembered for good (`dead`), so a late progress event or a stale snapshot never brings it back; an event with a revision newer than the snapshot makes `needsSnapshot` true.
 */
import { isBool, isNum, isStr, listOf, nullable, shaped } from "./ipc";

export type Phase = "fetching" | "verifying" | "installing";
export type Job = { generation: number; done: number; total: number; phase: Phase };
/** How the previous download of a model ended; kept by Rust until the model starts again or is deleted. */
export type Last = { generation: number; outcome: "succeeded" | "failed" | "cancelled"; error: string | null };

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
  /** How the previous download ended, if still remembered. */
  last: Last | null;
  /** Deleting now would clash with a running download. */
  deleteBlocked: boolean;
};
export type Snapshot = { revision: number; models: ModelInfo[] };
export type Progress = { id: string; generation: number; done: number; total: number };
export type Finished = { id: string; generation: number; revision: number; outcome: Last["outcome"]; error: string | null };
export type Changed = { revision: number };

export type Msg =
  | { type: "snapshot"; snapshot: Snapshot }
  | { type: "progress"; progress: Progress }
  | { type: "finished"; finished: Finished }
  | { type: "changed"; changed: Changed }
  | { type: "invalid" };

export type State = {
  /** Revision of the snapshot held; -1 before the first. */
  revision: number;
  /** Newest revision any message mentioned. */
  wanted: number;
  models: ModelInfo[];
  /** Percent of every running download. */
  live: Record<string, { generation: number; percent: number }>;
  /** Highest generation of each model known to be over. */
  dead: Record<string, number>;
  /** Progress heard before the first snapshot. */
  early: Progress[];
  /** True after a payload that could not be read, until a good snapshot lands. */
  unreadable: boolean;
};

/** Whether a value is a whole number that is not negative. */
const isCount = (v: unknown): v is number => isNum(v) && Number.isSafeInteger(v) && v >= 0;
/** A check for a value from a fixed list. Input: the allowed values. Return: the check. */
const among = <T>(values: T[]) => (v: unknown): v is T => new Set<unknown>(values).has(v);
/** Whether a value is a model kind. */
const isKind = among<ModelInfo["kind"]>(["speech", "text", "dictionary"]);
/** Whether a value is a session length. */
const isSessions = among<ModelInfo["sessions"]>(["long", "short", ""]);
/** Whether a value is a download phase. */
const isPhase = among<Phase>(["fetching", "verifying", "installing"]);
/** Whether a value is a download outcome. */
const isOutcome = among<Last["outcome"]>(["succeeded", "failed", "cancelled"]);

/** Whether `v` is a job: safe-integer generation and counts, known phase. */
const isJob = (v: unknown): v is Job => shaped<Job>(v, { generation: isCount, done: isCount, total: isCount, phase: isPhase });

/** Whether `v` is a last result. */
const isLast = (v: unknown): v is Last => shaped<Last>(v, { generation: isCount, outcome: isOutcome, error: nullable(isStr) });

/** Whether `v` is a model row with the types the cards use; `job` and `last` may be null. */
const isModel = (v: unknown): v is ModelInfo =>
  shaped<ModelInfo>(v, {
    id: isStr, kind: isKind, name: isStr, summary: isStr, languages: isStr, accuracy: isCount, speed: isCount, memoryMb: isCount,
    recommended: isBool, tight: isBool, sessions: isSessions, size: isCount, installed: isBool, job: nullable(isJob), last: nullable(isLast), deleteBlocked: isBool,
  });

/** A model row whose job's `done` beyond `total` is clamped to it, so one odd row never costs the whole list. */
const clamped = (m: ModelInfo): ModelInfo => (m.job ? { ...m, job: { ...m.job, done: Math.min(m.job.done, m.job.total) } } : m);

/** The snapshot `v` holds, or null when any part of it is malformed. */
export function parseSnapshot(v: unknown): Snapshot | null {
  return shaped<Snapshot>(v, { revision: isCount, models: listOf(isModel) }) ? { revision: v.revision, models: v.models.map(clamped) } : null;
}

/** The progress event `v` holds, or null (a `done` beyond `total` included). */
export function parseProgress(v: unknown): Progress | null {
  const ok = shaped<Progress>(v, { id: isStr, generation: isCount, done: isCount, total: isCount }) && v.done <= v.total;
  return ok ? { id: v.id, generation: v.generation, done: v.done, total: v.total } : null;
}

/** The finished event `v` holds, or null. */
export function parseFinished(v: unknown): Finished | null {
  if (!shaped<Finished>(v, { id: isStr, generation: isCount, revision: isCount, outcome: isOutcome, error: nullable(isStr) })) return null;
  return { id: v.id, generation: v.generation, revision: v.revision, outcome: v.outcome, error: v.error };
}

/** The changed event `v` holds, or null. */
export function parseChanged(v: unknown): Changed | null {
  return shaped<Changed>(v, { revision: isCount }) ? { revision: v.revision } : null;
}

/** What to tell the user about the previous download of `model`, while it still matters: a failure with its reason, or a cancel. Null while it downloads, once it is installed, after a success, and when nothing is remembered. */
export function lastResult(model: ModelInfo): string | null {
  if (model.job || model.installed || !model.last) return null;
  if (model.last.outcome === "failed") return model.last.error ?? "The download failed.";
  return model.last.outcome === "cancelled" ? "Download cancelled." : null;
}

/** Whole percent of `total` that `done` is; 0 when the size is not known. */
const percentOf = (done: number, total: number) => (total > 0 ? Math.floor((100 * done) / total) : 0);

/** The state before anything was heard. */
export const initial: State = { revision: -1, wanted: -1, models: [], live: {}, dead: {}, early: [], unreadable: false };

/** True when a message mentioned a revision newer than the snapshot held, so the snapshot must be read again. */
export const needsSnapshot = (s: State) => s.wanted > s.revision;

/** Percent of every running download, by model id. */
export const progressOf = (s: State): Record<string, number> => Object.fromEntries(Object.entries(s.live).map(([id, j]) => [id, j.percent]));

/** Counts a progress event unless its download is over or older than the one held. */
function progressed(s: State, p: Progress): State {
  const held = s.live[p.id];
  if (p.generation <= (s.dead[p.id] ?? -1) || (held && p.generation < held.generation)) return s;
  return { ...s, live: { ...s.live, [p.id]: { generation: p.generation, percent: percentOf(p.done, p.total) } } };
}

/** Ends the download a finish event belongs to, never a newer one, and remembers its generation for good. */
function concluded(s: State, f: Finished): State {
  const dead = { ...s.dead, [f.id]: Math.max(s.dead[f.id] ?? -1, f.generation) };
  const ends = (g?: number) => g !== undefined && g <= f.generation;
  const live = ends(s.live[f.id]?.generation) ? Object.fromEntries(Object.entries(s.live).filter(([id]) => id !== f.id)) : s.live;
  const result: Last = { generation: f.generation, outcome: f.outcome, error: f.error };
  const models = s.models.map((m) => {
    if (m.id !== f.id) return m;
    return { ...m, job: ends(m.job?.generation) ? null : m.job, last: f.generation >= (m.last?.generation ?? -1) ? result : m.last };
  });
  return { ...s, dead, live, models, wanted: Math.max(s.wanted, f.revision) };
}

/** Takes a snapshot unless it is not newer than the one held (an equal revision adds nothing and must not undo what events said since, such as a result),  drops downloads known to be over, then replays progress heard before it. */
function snapshotted(s: State, snap: Snapshot): State {
  if (snap.revision <= s.revision) return { ...s, unreadable: false };
  const dead = Object.fromEntries(snap.models.flatMap((m) => (m.last ? [[m.id, Math.max(s.dead[m.id] ?? -1, m.last.generation)]] : [])));
  const known = { ...s.dead, ...dead };
  const models = snap.models.map((m) => (m.job && m.job.generation <= (known[m.id] ?? -1) ? { ...m, job: null } : m));
  const live = Object.fromEntries(models.flatMap((m) => (m.job ? [[m.id, { generation: m.job.generation, percent: percentOf(m.job.done, m.job.total) }]] : [])));
  const base: State = { ...s, revision: snap.revision, models, live, dead: known, early: [], unreadable: false };
  return s.early.reduce(progressed, base);
}

/** The next state after one message; pure. */
export function reduce(s: State, m: Msg): State {
  switch (m.type) {
    case "snapshot": return snapshotted(s, m.snapshot);
    case "progress": return s.revision < 0 ? { ...s, early: [...s.early, m.progress] } : progressed(s, m.progress);
    case "finished": return concluded(s, m.finished);
    case "changed": return { ...s, wanted: Math.max(s.wanted, m.changed.revision) };
    case "invalid": return { ...s, unreadable: true };
  }
}
