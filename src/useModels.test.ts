/**
 * File: useModels.test.ts
 * Purpose: Protect the Models page's connection: a listener Rust refuses marks the catalog unreadable instead of spinning forever, Retry subscribes again and the catalog appears, two Retry clicks open one subscription, and leaving the page removes the listeners.
 * Contents: the refused-subscription then retry case and the double-Retry case, over the Tauri IPC mock.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { afterEach, describe, expect, it } from "vitest";
import { useModels } from "./useModels";

afterEach(() => clearMocks());

/** Waits for pending promises. */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

const ROW = {
  id: "a", kind: "speech", name: "a", summary: "", languages: "en", accuracy: 1, speed: 1, memoryMb: 0, recommended: false, tight: false, sessions: "", size: 100, installed: false, job: null, last: null, deleteBlocked: false,
};

describe("useModels", () => {
  it("shows a refused subscription as unreadable, reconnects on Retry and stops on unmount", async () => {
    let refuse = true;
    let unlistens = 0;
    mockIPC((cmd) => {
      if (cmd === "plugin:event|listen") { if (refuse) throw new Error("denied"); return 1; }
      if (cmd === "plugin:event|unlisten") unlistens += 1;
      if (cmd === "list_models") return { revision: 1, models: [ROW] };
      return undefined;
    });
    const { m, dispose } = createRoot((dispose) => ({ m: useModels(), dispose }));
    await flush();
    expect(m.unreadable()).toBe(true);
    refuse = false;
    m.retry();
    await flush();
    expect(m.unreadable()).toBe(false);
    expect(m.models().map((r) => r.id)).toEqual(["a"]);
    dispose();
    await flush();
    expect(unlistens).toBe(3);
  });

  it("opens one subscription for two Retry clicks, so a slower duplicate cannot replace the catalog", async () => {
    let refuse = true;
    let reads = 0;
    mockIPC((cmd) => {
      if (cmd === "plugin:event|listen") { if (refuse) throw new Error("denied"); return 1; }
      if (cmd === "list_models") { reads += 1; return { revision: reads, models: [{ ...ROW, name: `read ${reads}` }] }; }
      return undefined;
    });
    const { m, dispose } = createRoot((dispose) => ({ m: useModels(), dispose }));
    await flush();
    refuse = false;
    m.retry();
    m.retry();
    await flush();
    expect(reads).toBe(1);
    expect(m.models().map((r) => r.name)).toEqual(["read 1"]);
    dispose();
  });
});
