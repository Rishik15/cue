/**
 * File: templates.test.ts
 * Purpose: Keeps the starter templates honest: each is named in at most four words and every action it uses has a sample text, so the Test panel is never empty.
 * Contents: one case over TEMPLATES and sampleFor, plus the exact actions without a template.
 */
import { describe, expect, it } from "vitest";
import { newRule } from "./rules";
import { sampleFor, TEMPLATES } from "./templates";

describe("templates", () => {
  it("are named in four words or fewer, and every action they use has a sample", () => {
    for (const t of TEMPLATES) {
      expect(t.name.split(/\s+/).length, t.name).toBeLessThanOrEqual(4);
      for (const action of t.actions) expect(sampleFor({ ...newRule(), actions: [action] }), `${t.name}: ${action}`).not.toBe("");
    }
  });

  it("have a sample for quote and unquote, which no template starts from", () => {
    for (const action of ["quote", "unquote"]) expect(sampleFor({ ...newRule(), actions: [action] })).not.toBe("");
  });
});
