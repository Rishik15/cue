/**
 * File: ipc.test.ts
 * Purpose: The shared IPC checks: every field of a shape is checked, lists check every element, `nullable` lets null through, and `read` rejects a malformed reply with a readable message while passing a refusal on.
 * Contents: cases for shaped, listOf / strings, oneOf, nullable, read and reason.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { isNum, isStr, nullable, oneOf, read, reason, shaped, strings } from "./ipc";

afterEach(() => clearMocks());

/** A small shape for the cases. */
type Pair = { name: string; size: number };
/** The check the read cases use. */
const isPair = (raw: unknown): raw is Pair => shaped<Pair>(raw, { name: isStr, size: isNum });

describe("checks", () => {
  it("shaped needs every field to pass and an object", () => {
    expect(isPair({ name: "a", size: 1 })).toBe(true);
    expect([isPair({ name: "a", size: "1" }), isPair({ name: "a" }), isPair([]), isPair(null)]).toEqual([false, false, false, false]);
  });

  it("strings checks every element, oneOf only the table's values", () => {
    expect([strings(["a", "b"]), strings(["a", 1]), strings("a")]).toEqual([true, false, false]);
    expect([oneOf({ Dark: "dark" }, "dark"), oneOf({ Dark: "dark" }, "Dark")]).toEqual([true, false]);
  });

  it("nullable lets null and the checked value through, nothing else", () => {
    expect([nullable(isNum)(null), nullable(isNum)(2), nullable(isNum)("2"), nullable(isNum)(undefined)]).toEqual([true, true, false, false]);
  });
});

describe("read", () => {
  it("resolves a valid reply, rejects a malformed one and passes a refusal on", async () => {
    let reply: () => unknown = () => ({ name: "a", size: 1 });
    mockIPC(() => reply());
    await expect(read("get_pair", isPair, "Cue could not read the pair.")).resolves.toEqual({ name: "a", size: 1 });
    reply = () => ({ name: 1 });
    await expect(read("get_pair", isPair, "Cue could not read the pair.")).rejects.toThrow("Cue could not read the pair.");
    reply = () => { throw new Error("refused"); };
    await expect(read("get_pair", isPair, "Cue could not read the pair.")).rejects.toThrow("refused");
  });

  it("resolves null only when the check allows it", async () => {
    mockIPC(() => null);
    await expect(read("get_pair", nullable(isPair), "Cue could not read the pair.")).resolves.toBeNull();
    await expect(read("get_pair", isPair, "Cue could not read the pair.")).rejects.toThrow("Cue could not read the pair.");
  });

  it("reason shows an Error's message without a prefix and a refusal as it came", () => {
    expect([reason(new Error("Cue could not read the pair.")), reason("refused")]).toEqual(["Cue could not read the pair.", "refused"]);
  });
});
