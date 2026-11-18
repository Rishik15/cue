/**
 * File: names.test.ts
 * Purpose: Which lines define a name and where names are used, read the way the calculator reads them.
 * Contents: tests of analyse.
 */
import { Text } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { analyse } from "./names";

/** The names defined in a text. Input: the text. Return: the names. */
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
  it("finds a definition made of a function or a constant", () => {
    expect(names("rent: sqrt(900)")).toEqual(["rent"]);
    expect(names("rent: pi")).toEqual(["rent"]);
    expect(names("rent: -ln(2) * 3")).toEqual(["rent"]);
  });
  it("finds a definition made of a function without brackets, a cube root or a leading point", () => {
    expect(names("rent: sqrt 900")).toEqual(["rent"]);
    expect(names("rent: ∛8")).toEqual(["rent"]);
    expect(names("rent: .5")).toEqual(["rent"]);
  });
  it("finds a definition made of a function and a name, a signed number or a constant", () => {
    expect(names("a: 9\nrent: sqrt a")).toEqual(["a", "rent"]);
    expect(names("rent: abs -9")).toEqual(["rent"]);
    expect(names("rent: sqrt pi")).toEqual(["rent"]);
    expect(names("rent: sqrt abs -9")).toEqual(["rent"]);
  });
  it("does not take a function stuck to digits or with no argument for a definition", () => {
    expect(names("rent: sqrt900")).toEqual([]);
    expect(names("rent: sqrt")).toEqual([]);
    expect(names("rent: sqrt of the thing")).toEqual([]);
  });
  it("does not take prose with a colon for a definition", () => {
    expect(names("call back: later")).toEqual([]);
  });
  it("does not use a name above its definition", () => {
    expect(analyse(Text.of(["rent / 3 =", "rent: 900"])).refs.size).toBe(0);
  });
});
