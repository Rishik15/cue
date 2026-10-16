/**
 * Purpose: The shapes the dictionary commands send, checked where they enter the page.
 * Contents: Sense / Block / Definition — a looked-up word; Saved — a saved word; toDefinition / toSaved — shape checks for data from `invoke` (anything else becomes null).
 */
export type Sense = { gloss: string; example?: string };
export type Block = { word: string; pos: string; ipa?: string; senses: Sense[]; synonyms?: string[] };
export type Definition = { word: string; language: string; blocks: Block[]; saved: boolean };
export type Saved = { id: number; word: string; pos: string; ipa: string; gloss: string; example: string; language: string; added: number };

const isText = (v: unknown): v is string => typeof v === "string";
const isSense = (v: unknown): v is Sense => !!v && isText((v as Sense).gloss) && ((v as Sense).example === undefined || isText((v as Sense).example));
const isBlock = (v: unknown): v is Block =>
  !!v && isText((v as Block).word) && isText((v as Block).pos) && Array.isArray((v as Block).senses) && (v as Block).senses.every(isSense);

export function toDefinition(v: unknown): Definition | null {
  const d = v as Definition | null;
  return d && isText(d.word) && isText(d.language) && Array.isArray(d.blocks) && d.blocks.every(isBlock) && d.blocks.length > 0 ? d : null;
}

export const toSaved = (v: unknown): Saved[] => (Array.isArray(v) ? v.filter((s): s is Saved => !!s && isText((s as Saved).word) && isText((s as Saved).gloss)) : []);
