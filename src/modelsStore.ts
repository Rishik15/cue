/**
 * File: src/modelsStore.ts
 * Purpose: The one place that decides what the Models page shows while downloads start, move and end from several windows: a reducer over snapshots and events, plus the guards that reject anything Rust did not mean to send.
 * Contents: ModelInfo / Job / Snapshot — the data; parseSnapshot / parseProgress / parseFinished / parseChanged — IPC guards returning null for a bad shape; reduce / initial / needsSnapshot / progressOf — the state machine and its reads.
 * Rules: a snapshot older than the one held is ignored; progress that arrives before the first snapshot is kept and replayed after it; a finished generation of a model is remembered for good (`dead`), so a late progress event or a stale snapshot never brings it back; an event with a revision newer than the snapshot makes `needsSnapshot` true.
 */

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
export type Finished = { id: string; generation: number; revision: number; outcome: string; error: string | null };
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

type Obj = Record<string, unknown>;
const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);
const isCount = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const isText = (v: unknown): v is string => typeof v === "string";
const KINDS = ["speech", "text", "dictionary"];
const SESSIONS = ["long", "short", ""];

const PHASES = ["fetching", "verifying", "installing"];
const OUTCOMES = ["succeeded", "failed", "cancelled"];

/** A job when `v` is a well-formed one: safe-integer generation, known phase, `done` within 0..total. */
function parseJob(v: unknown): Job | null {
  if (!isObj(v) || !isCount(v.generation) || !isCount(v.done) || !isCount(v.total) || v.done > v.total || !PHASES.includes(v.phase as string)) return null;
  return { generation: v.generation, done: v.done, total: v.total, phase: v.phase as Phase };
}

/** The last result when `v` is a well-formed one. */
function parseLast(v: unknown): Last | null {
  if (!isObj(v) || !isCount(v.generation) || !OUTCOMES.includes(v.outcome as string) || (v.error !== null && !isText(v.error))) return null;
  return { generation: v.generation, outcome: v.outcome as Last["outcome"], error: v.error };
}

/** Whether the plain fields of a model row have the types the cards use. */
function plainFieldsOk(v: Obj): boolean {
  const textOk = [v.id, v.name, v.summary, v.languages].every(isText);
  const countOk = [v.accuracy, v.speed, v.memoryMb, v.size].every(isCount);
  const flagOk = [v.recommended, v.tight, v.installed, v.deleteBlocked].every((f) => typeof f === "boolean");
  return textOk && countOk && flagOk && KINDS.includes(v.kind as string) && SESSIONS.includes(v.sessions as string);
}

/** A model row when `v` is well-formed; `job` and `last` may be null. */
function parseModel(v: unknown): ModelInfo | null {
  if (!isObj(v) || !plainFieldsOk(v)) return null;
  const job = v.job === null ? null : parseJob(v.job);
  const last = v.last === null ? null : parseLast(v.last);
  if ((v.job !== null && !job) || (v.last !== null && !last)) return null;
  return { ...(v as unknown as ModelInfo), job, last };
}

/** The snapshot `v` holds, or null when any part of it is malformed. */
export function parseSnapshot(v: unknown): Snapshot | null {
  if (!isObj(v) || !isCount(v.revision) || !Array.isArray(v.models)) return null;
  const models = v.models.map(parseModel);
  return models.every((m) => m) ? { revision: v.revision, models: models as ModelInfo[] } : null;
}

/** The progress event `v` holds, or null. */
export function parseProgress(v: unknown): Progress | null {
  if (!isObj(v) || !isText(v.id) || !isCount(v.generation) || !isCount(v.done) || !isCount(v.total) || v.done > v.total) return null;
  return { id: v.id, generation: v.generation, done: v.done, total: v.total };
}

/** The finished event `v` holds, or null. */
export function parseFinished(v: unknown): Finished | null {
  if (!isObj(v) || !isText(v.id) || !isCount(v.generation) || !isCount(v.revision) || !isText(v.outcome)) return null;
  if (v.error !== null && !isText(v.error)) return null;
  return { id: v.id, generation: v.generation, revision: v.revision, outcome: v.outcome, error: v.error };
}

/** The changed event `v` holds, or null. */
export function parseChanged(v: unknown): Changed | null {
  return isObj(v) && isCount(v.revision) ? { revision: v.revision } : null;
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
  const models = s.models.map((m) => (m.id === f.id && ends(m.job?.generation) ? { ...m, job: null } : m));
  return { ...s, dead, live, models, wanted: Math.max(s.wanted, f.revision) };
}

/** Takes a snapshot unless it is older than the one held, drops downloads known to be over, then replays progress heard before it. */
function snapshotted(s: State, snap: Snapshot): State {
  if (snap.revision < s.revision) return s;
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
