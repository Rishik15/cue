/**
 * File: ui/latest.test.ts
 * Purpose: Protect latest-wins: only the newest request's check holds, an older one that settles late is dropped.
 * Contents: cases for latest.
 */
import { describe, expect, it } from "vitest";
import { latest } from "./latest";

describe("latest", () => {
  it("keeps only the newest request current", () => {
    const begin = latest();
    const first = begin();
    expect(first()).toBe(true);
    const second = begin();
    expect(first()).toBe(false);
    expect(second()).toBe(true);
    expect(second.seq).toBeGreaterThan(first.seq);
  });

  it("counts each stream on its own", () => {
    const [a, b] = [latest(), latest()];
    const mine = a();
    b();
    expect(mine()).toBe(true);
  });
});
