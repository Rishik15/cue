/**
 * File: preview/usePalette.test.ts
 * Purpose: Protect the palette's sessions through the Tauri boundary: a pick asked under one grab is never applied under the next one when the reused window gets new text, and a new grab starts with clean results, filter and highlight.
 * Contents: cases for usePalette with `preview_run` gated and `preview_apply` recorded.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot, createSignal } from "solid-js";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Item } from "./parts";
import { usePalette } from "./usePalette";

/** A palette row; `result` null makes it a text-model row. Input: id and result. Return: the row. */
const row = (id: string, result: string | null): Item => ({ id, name: id, hint: "", group: result === null ? "ai" : "clean", model: result === null, result });

let dispose: (() => void) | undefined;
afterEach(() => { dispose?.(); clearMocks(); });

/** Mounts the hook with the Tauri commands mocked: each `preview_run` waits until released. Inputs: first text and rows. Return: the hook, its setters, the calls and the waiting runs. */
function mount(original: string, rows: Item[]) {
  const calls: [string, Record<string, unknown>][] = [];
  const runs: ((out: string | null) => void)[] = [];
  mockIPC((cmd, args) => {
    calls.push([cmd, args as Record<string, unknown>]);
    return cmd === "preview_run" ? new Promise((r) => runs.push(r)) : undefined;
  });
  return createRoot((stop) => {
    dispose = stop;
    const [text, setText] = createSignal(original);
    const [items, setItems] = createSignal(rows);
    const [session, setSession] = createSignal(1);
    /** Publishes a new grab into the reused window, as `preview-changed` does. Inputs: its session, text and rows. */
    const publish = (id: number, next: string, nextRows: Item[]) => { setText(next); setItems(nextRows); setSession(id); };
    return { s: usePalette(text, items, session), publish, calls, runs };
  });
}

describe("usePalette sessions", () => {
  it("a_stale_pick_is_not_applied_under_a_new_session", async () => {
    const { s, publish, calls, runs } = mount("text A", [row("ai.fix", null)]);
    const picked = s.paste();
    await vi.waitFor(() => expect(runs.length).toBe(1));
    publish(2, "text B", [row("ai.fix", null)]);
    runs[0]?.("fixed A");
    await picked;
    const applied = calls.filter((c) => c[0] === "preview_apply").map((c) => c[1].session);
    expect(applied.every((id) => id === 1)).toBe(true);
    expect(s.text()).not.toBe("fixed A");
  });

  it("a_new_session_starts_clean", async () => {
    const { s, publish, calls, runs } = mount("text A", [row("clean.trim", "A trimmed"), row("ai.fix", null)]);
    s.move(1);
    await vi.waitFor(() => expect(runs.length).toBe(1));
    s.setQuery("tri");
    s.edit("my edit");
    publish(2, "text B", [row("clean.trim", "B trimmed"), row("ai.fix", null)]);
    expect(s.query()).toBe("");
    expect(s.active()).toBe(0);
    expect(s.text()).toBe("B trimmed");
    s.move(1);
    await vi.waitFor(() => expect(runs.length).toBe(2));
    expect(calls.filter((c) => c[0] === "preview_run").map((c) => c[1].session)).toEqual([1, 2]);
    runs[0]?.("fixed A");
    runs[1]?.("fixed B");
    await vi.waitFor(() => expect(s.text()).toBe("fixed B"));
  });
});
