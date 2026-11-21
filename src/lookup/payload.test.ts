/**
 * File: lookup/payload.test.ts
 * Purpose: Protect the checks on what Rust sends the dictionary window: a good open event, entry, hits and placement pass whole, and each kind of malformed one (bad session, unknown mode, an entry with no meanings or a sense of the wrong type, a hit missing a field, a non-finite size) is refused.
 * Contents: cases for asOpen, asEntry, asHits and asPlacement.
 */
import { describe, expect, it } from "vitest";
import { asEntry, asHits, asOpen, asPlacement } from "./payload";

const ENTRY = { key: "serene", language: "English", blocks: [{ word: "serene", pos: "adjective", senses: [{ gloss: "Calm.", example: "A serene lake." }, { gloss: "Clear." }] }] };
const CARD = { session: 3, mode: "card", theme: { theme_mode: "Dark" }, entry: ENTRY };
const SEARCH = { session: 4, mode: "search", theme: {}, recent: [{ word: "lucid", pos: "adjective" }], pack: true };

describe("asOpen", () => {
  it("accepts a card and a search window", () => {
    expect(asOpen(CARD)).toEqual(CARD);
    expect(asOpen(SEARCH)).toEqual(SEARCH);
    expect(asOpen({ ...SEARCH, recent: [] })).not.toBeNull();
  });

  it("refuses each malformed part", () => {
    const bad = [
      null,
      "card",
      { ...CARD, session: 0 },
      { ...CARD, session: 1.5 },
      { ...CARD, mode: "other" },
      { ...CARD, theme: { theme_mode: 3 } },
      { ...CARD, entry: { ...ENTRY, blocks: [] } },
      { ...CARD, entry: { ...ENTRY, blocks: [{ word: "x", pos: "noun", senses: [{ gloss: 5 }] }] } },
      { ...SEARCH, pack: "yes" },
      { ...SEARCH, recent: [{ word: "x" }] },
    ];
    for (const value of bad) expect(asOpen(value), JSON.stringify(value)).toBeNull();
  });
});

describe("asEntry, asHits and asPlacement", () => {
  it("checks an entry", () => {
    expect(asEntry(ENTRY)).toEqual(ENTRY);
    expect(asEntry({ ...ENTRY, language: 1 })).toBeNull();
    expect(asEntry(null)).toBeNull();
  });

  it("keeps good hits and drops the whole list for one bad row", () => {
    const hit = { key: "serif", word: "serif", pos: "noun", gloss: "A stroke." };
    expect(asHits([hit])).toEqual([hit]);
    expect(asHits([hit, { key: "x" }])).toEqual([]);
    expect(asHits("nope")).toEqual([]);
  });

  it("accepts a block with no synonyms or string synonyms and refuses malformed ones", () => {
    const block = ENTRY.blocks[0];
    expect(asEntry({ ...ENTRY, blocks: [block] })).not.toBeNull();
    expect(asEntry({ ...ENTRY, blocks: [{ ...block, synonyms: ["calm", "still"] }] })).not.toBeNull();
    for (const synonyms of ["calm", [1], ["calm", null], {}]) expect(asEntry({ ...ENTRY, blocks: [{ ...block, synonyms }] }), JSON.stringify(synonyms)).toBeNull();
  });

  it("checks a placement", () => {
    const good = { side: "below", arrow: 40, w: 300, h: 180 };
    expect(asPlacement(good)).toEqual(good);
    expect(asPlacement({ ...good, arrow: null, side: "centre" })).not.toBeNull();
    for (const bad of [{ ...good, side: "up" }, { ...good, w: 0 }, { ...good, h: Number.NaN }, { ...good, arrow: "3" }, null]) {
      expect(asPlacement(bad)).toBeNull();
    }
  });
});
