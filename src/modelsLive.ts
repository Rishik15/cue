/**
 * Purpose: The rules that keep a Models page right while downloads start, move and end from several places: which events still count.
 * Contents: Live — the downloads a page knows about; fromSnapshot / progressed / finished — pure updates; behind — whether an event is newer than the snapshot held.
 * Every event carries the generation of its download, so the old download of a model cancelled and started again can never overwrite the new one.
 */

/** `ended` keeps the generation of a finished download as a floor, so a late progress event of it cannot bring it back. */
export type Live = Record<string, { generation: number; percent: number; ended?: boolean }>;
export type Job = { generation: number; done: number; total: number };

/** Whole percent of `total` that `done` is; 0 when the size is not known. */
const percentOf = (done: number, total: number) => (total > 0 ? Math.floor((100 * done) / total) : 0);

/** The downloads a snapshot lists; replaces everything held. */
export function fromSnapshot(models: { id: string; job: Job | null }[]): Live {
  return Object.fromEntries(models.flatMap((m) => (m.job ? [[m.id, { generation: m.job.generation, percent: percentOf(m.job.done, m.job.total) }]] : [])));
}

/** A progress event: counted unless it belongs to an older download of the same model. */
export function progressed(live: Live, e: { id: string; generation: number; done: number; total: number }): Live {
  const held = live[e.id];
  if (held && (e.generation < held.generation || (held.ended && e.generation === held.generation))) return live;
  return { ...live, [e.id]: { generation: e.generation, percent: percentOf(e.done, e.total) } };
}

/** A finish event: ends the download it belongs to, never a newer one, and remembers its generation. */
export function finished(live: Live, e: { id: string; generation: number }): Live {
  const held = live[e.id];
  if (held && e.generation < held.generation) return live;
  return { ...live, [e.id]: { generation: e.generation, percent: 0, ended: true } };
}

/** True when an event is from after the snapshot held, so the snapshot must be read again. */
export function behind(snapshotRevision: number, eventRevision: number): boolean {
  return eventRevision > snapshotRevision;
}
