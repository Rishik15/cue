/**
 * File: plain.test.ts
 * Purpose: What the paste clean-up keeps and removes: joiners that shape words and emoji stay, hidden marks go, odd spaces become plain.
 * Contents: tests of plain.
 */
import { describe, expect, it } from "vitest";
import { plain } from "./plain";

describe("plain", () => {
  it("keeps the zero-width joiner of an emoji sequence", () => {
    expect(plain("👩‍💻")).toBe("👩‍💻");
  });
  it("keeps the zero-width non-joiner of a Persian word", () => {
    expect(plain("می‌روم")).toBe("می‌روم");
  });
  it("removes zero-width space, direction marks and the byte order mark", () => {
    expect(plain("a​b‎c‪d﻿e")).toBe("abcde");
  });
  it("turns no-break and ideographic spaces into plain spaces", () => {
    expect(plain("a b　c")).toBe("a b c");
  });
  it("returns clean text as it is", () => {
    expect(plain("rent: 900")).toBe("rent: 900");
  });
});
