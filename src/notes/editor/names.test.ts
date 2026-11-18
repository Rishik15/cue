/**
 * File: names.test.ts
 * Purpose: Which lines define a name and where names are used, read the way the calculator reads them.
 * Contents: tests of analyse.
 */
import { Text } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { analyse } from "./names";

const names = (s: string) => analyse(Text.of(s.split("\n"))).defs.map((d) => d.name);

describe("analyse", () => {
  it("finds a plain definition and a use below it", () => {
    const a = analyse(Text.of(["rent: 900", "rent / 3 ="]));
    expect(a.defs.map((d) => d.name)).toEqual(["rent"]);
    expect([...a.refs.keys()]).toEqual([2]);
  });
  it("finds a definition behind a numbered, bullet or box marker", () => {
    expect(names("1. rent: 900")).toEqual(["rent"]);
    expect(names("2) rent: 900")).toEqual(["rent"]);
    expect(names("- rent: 900")).toEqual(["rent"]);
    expect(names("[ ] rent: 900")).toEqual(["rent"]);
  });
  it("does not take prose with a colon for a definition", () => {
    expect(names("call back: later")).toEqual([]);
  });
  it("does not use a name above its definition", () => {
    expect(analyse(Text.of(["rent / 3 =", "rent: 900"])).refs.size).toBe(0);
  });
});
