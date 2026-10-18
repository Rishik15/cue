/**
 * Purpose: The shape the Looked Up list's command sends, checked where it enters the page.
 * Contents: Saved — a word in the Looked Up list; toSaved — shape check for data from `invoke` (a row with any field of the wrong type is dropped).
 */
export type Saved = { id: number; word: string; pos: string; gloss: string; example: string; language: string; added: number };

const isText = (v: unknown): v is string => typeof v === "string";
const isNumber = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);

const isSaved = (v: unknown): v is Saved => {
  const s = v as Saved | null;
  return !!s && isNumber(s.id) && isText(s.word) && isText(s.pos) && isText(s.gloss) && isText(s.example) && isText(s.language) && isNumber(s.added);
};

export const toSaved = (v: unknown): Saved[] => (Array.isArray(v) ? v.filter(isSaved) : []);
