/**
 * File: Editor.test.ts
 * Purpose: The editor and the note hook together: no typing is possible from the ask for a move until the new note shows, so nothing typed can reach a note that is being left.
 * Contents: tests of Editor with useNote.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createRoot } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Options } from "../types";
import { useNote } from "../useNote";
import { Editor } from "./Editor";

const options: Options = { text_size: 14, paper: "plain", checked: "x", digits: 2, thousands: true };

afterEach(() => { clearMocks(); document.body.innerHTML = ""; });

describe("Editor while a move is out", () => {
  it("is not editable from the ask for a move until the new note shows", async () => {
    let release!: (v: unknown) => void;
    const slow = new Promise((r) => { release = r; });
    mockIPC((name, args) => {
      const cmd = (args as { cmd: { do: string } }).cmd;
      if (name === "note" && cmd.do === "open") return slow;
      return undefined;
    });
    let hook!: ReturnType<typeof useNote>;
    const dispose = createRoot((d) => {
      hook = useNote({ note: { id: 1, body: "", pinned: false } }, () => undefined);
      render(() => <Editor id={hook.open().note.id} body={hook.open().note.body} keys={[]} options={options} locked={hook.moving()} onEdit={hook.edit} onInfo={() => undefined} api={() => undefined} onTimerLine={() => undefined} />, document.body);
      return d;
    });
    const content = () => document.querySelector(".cm-content")?.getAttribute("contenteditable");
    expect(content()).toBe("true");
    const moving = hook.pick(2);
    await vi.waitFor(() => expect(content()).toBe("false"));
    release({ note: { id: 2, body: "b", pinned: false } });
    await moving;
    await vi.waitFor(() => expect(content()).toBe("true"));
    dispose();
  });
});
