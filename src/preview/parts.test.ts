/**
 * File: preview/parts.test.ts
 * Purpose: Protect the preview's close: it names the session the page shows and has no fallback that closes the window by itself, so a refused close never destroys newer text.
 * Contents: closePreview over the Tauri IPC mock: the arguments it sends and a rejected call.
 */
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { closePreview } from "./parts";

afterEach(() => clearMocks());

describe("closePreview", () => {
  it("sends the session and the text to preview_close", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((cmd, args) => void calls.push([cmd, args]));
    await closePreview(7, "x");
    expect(calls).toEqual([["preview_close", { session: 7, text: "x" }]]);
  });

  it("sends a null text when the box has none", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((cmd, args) => void calls.push([cmd, args]));
    await closePreview(3);
    expect(calls).toEqual([["preview_close", { session: 3, text: null }]]);
  });

  it("does not touch the window when Rust refuses", async () => {
    const cmds: string[] = [];
    mockWindows("preview");
    mockIPC((cmd) => {
      cmds.push(cmd);
      throw new Error("refused");
    });
    await expect(closePreview(7, "x")).rejects.toBeDefined();
    expect(cmds).toEqual(["preview_close"]);
  });
});
