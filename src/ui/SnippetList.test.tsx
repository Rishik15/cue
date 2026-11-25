/**
 * File: ui/SnippetList.test.tsx
 * Purpose: Protect the snippet list over the library hook: a folded folder stays folded and a focused row keeps focus after a write re-reads the library, and the shown snippets are grouped by folder once, with the same groups a per-folder filter gives.
 * Contents: cases for SnippetList with `list_snippets` mocked.
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { draftOf, EMPTY, useSnippets, type Folder, type Snippet } from "../snippets";
import { SnippetList } from "./SnippetList";

let stop: (() => void) | undefined;
afterEach(() => { stop?.(); clearMocks(); document.body.innerHTML = ""; });

/** Waits for pending promises. */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

/** A stored snippet with the given id, name and folder. */
const snippet = (id: number, name: string, folder_id: number | null): Snippet => ({ ...EMPTY, id, name, body: name, folder_id, uses: 0, last_used: 0 });
/** A stored folder with the given id and name. */
const folder = (id: number, name: string): Folder => ({ id, name, enabled: true, only_apps: [], skip_apps: [] });

const NOOP = () => undefined;
const ACTIONS = { onNewSnippet: NOOP, onNewFolder: NOOP, onFromClipboard: NOOP, onEditFolder: NOOP, onEnableFolder: NOOP, onOpen: NOOP, onImport: NOOP, onExport: NOOP, onStarter: NOOP };

/** The button whose text includes `text`. */
const button = (text: string) => [...document.querySelectorAll("button")].find((b) => b.textContent?.includes(text)) as HTMLButtonElement;

describe("SnippetList", () => {
  it("keeps a folded folder folded and a focused row focused after a write", async () => {
    let name = "Greeting";
    mockIPC((cmd, args) => {
      if (cmd === "save_snippet") name = (args as { draft: Snippet }).draft.name;
      if (cmd === "list_snippets") return { snippets: [snippet(1, name, 2), snippet(3, "Loose", null)], folders: [folder(2, "Work")] };
      return undefined;
    });
    let lib!: ReturnType<typeof useSnippets>;
    stop = render(() => {
      lib = useSnippets();
      return <SnippetList {...ACTIONS} snippets={lib.snippets()} folders={lib.folders()} message="" />;
    }, document.body);
    await flush();
    button("Work").click();
    expect(button("Work").getAttribute("aria-expanded")).toBe("false");
    const row = button("Loose");
    row.focus();
    const [first] = lib.snippets();
    expect(first && (await lib.store(1, { ...draftOf(first), name: "Hello" }))).toBe(true);
    expect(button("Work").getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(row);
    button("Work").click();
    expect(button("Hello")).toBeDefined();
  });

  it("groups the shown snippets by folder once, as a per-folder filter would", () => {
    let reads = 0;
    const counted = (s: Snippet) => {
      const { folder_id, ...rest } = s;
      return Object.defineProperty(rest, "folder_id", { get: () => { reads += 1; return folder_id; }, enumerable: true }) as Snippet;
    };
    const folders = [folder(1, "Work"), folder(2, "Home"), folder(3, "Empty")];
    const snippets = [snippet(10, "a", 1), snippet(11, "b", 2), snippet(12, "c", 1), snippet(13, "d", null)].map(counted);
    stop = render(() => <SnippetList {...ACTIONS} snippets={snippets} folders={folders} message="" />, document.body);
    expect(reads).toBe(snippets.length);
    const count = (name: string) => button(name).querySelector(".t-meta")?.textContent;
    expect([count("Work"), count("Home"), count("Empty")]).toEqual(["2", "1", "0"]);
    expect(button("d")).toBeDefined();
  });
});
