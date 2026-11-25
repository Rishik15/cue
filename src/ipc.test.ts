/**
 * File: ipc.test.ts
 * Purpose: The shared IPC checks: every field of a shape is checked, lists check every element, and `read` rejects a malformed reply with a readable message while passing a refusal on.
 * Contents: cases for shaped, listOf / strings, oneOf and read.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { isNum, isStr, oneOf, read, shaped, strings } from "./ipc";

afterEach(() => clearMocks());

/** A small shape for the cases. */
type Pair = { name: string; size: number };
/** The parser the read cases use. */
const parsePair = (raw: unknown): Pair | null => (shaped<Pair>(raw, { name: isStr, size: isNum }) ? raw : null);

describe("checks", () => {
  it("shaped needs every field to pass and an object", () => {
    expect(parsePair({ name: "a", size: 1 })).toEqual({ name: "a", size: 1 });
    expect([parsePair({ name: "a", size: "1" }), parsePair({ name: "a" }), parsePair([]), parsePair(null)]).toEqual([null, null, null, null]);
  });

  it("strings checks every element, oneOf only the table's values", () => {
    expect([strings(["a", "b"]), strings(["a", 1]), strings("a")]).toEqual([true, false, false]);
    expect([oneOf({ Dark: "dark" }, "dark"), oneOf({ Dark: "dark" }, "Dark")]).toEqual([true, false]);
  });
});

describe("read", () => {
  it("resolves a valid reply, rejects a malformed one and passes a refusal on", async () => {
    let reply: () => unknown = () => ({ name: "a", size: 1 });
    mockIPC(() => reply());
    await expect(read("get_pair", parsePair)).resolves.toEqual({ name: "a", size: 1 });
    reply = () => ({ name: 1 });
    await expect(read("get_pair", parsePair)).rejects.toThrow("Cue could not read the answer to get_pair.");
    reply = () => { throw new Error("refused"); };
    await expect(read("get_pair", parsePair)).rejects.toThrow("refused");
  });
});
