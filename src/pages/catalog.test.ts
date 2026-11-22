/**
 * File: pages/catalog.test.ts
 * Purpose: Keep the page catalog, the setting search and the pages in step: every search entry belongs to a page of the catalog and has a `data-setting` target on that page (so opening it can scroll and flash), every page renders, and a page name from outside is accepted only when it is a page.
 * Contents: mountPage — render one page with the IPC mocked and wait for its targets; cases for ids, targets, rendering and parsePage.
 */
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createComponent } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SETTINGS } from "../search";
import { TitleBar } from "../ui/TitleBar";
import { go } from "../nav";
import { PAGES, parsePage, type PageEntry, type PageId } from "./catalog";

// theme.ts asks the media query when it loads, which jsdom does not have.
vi.hoisted(() => {
  window.matchMedia = ((query: string) => ({ matches: false, media: query, addEventListener() {}, removeEventListener() {} })) as never;
});

// jsdom has no scrollIntoView, which lists call on their highlighted row.
Element.prototype.scrollIntoView = () => undefined;

let stop: (() => void) | undefined;
/** How long a lazily loaded page may take to show its settings on a cold cache. */
const READY_MS = 10_000;

afterEach(() => {
  stop?.();
  document.body.innerHTML = "";
});

/** A model row of `kind`. */
const model = (kind: string) => ({ id: kind, kind, name: kind, summary: "", languages: "en", accuracy: 1, speed: 1, memoryMb: 0, recommended: false, tight: false, sessions: "", size: 100, installed: false, job: null, last: null, deleteBlocked: false });
/** A shortcut row of the catalog. */
const shortcut = (id: string) => ({ id, label: id, parts: [{ code: "ctrl", label: "Ctrl", side: "L" }], enabled: true, held: false, error: null });
/** A snippet, so the Snippets page shows its settings rather than the empty page. */
const snippet = { id: 1, name: "a", triggers: ["a"], body: "b", uses: 0, last_used: 0, folder_id: null, mode: "auto", match_case: false, enabled: true, insert: "paste", ending: "", inside_word: false, propagate_case: false, only_apps: [], skip_apps: [] };

/** What Rust would answer, in the smallest form each page needs. Input: the command. Return: its answer. */
const answer = (cmd: string): unknown => {
  if (cmd === "list_models") return { revision: 1, models: ["speech", "text", "dictionary"].map(model) };
  if (cmd === "shortcut_catalog") return { revision: 1, health: "ok", last_error: null, shortcuts: ["dictation", "selection", "grab", "define", "notes"].map(shortcut) };
  if (cmd === "list_snippets") return { snippets: [snippet], folders: [] };
  if (cmd === "get_stats") return { wpm: 0, words: 0, speakingSecs: 0, savedSecs: 0, days: [] };
  return [];
};

/** Renders `page` (and the header's feature switch, which lives outside the page) with Rust's answers mocked, and waits until the page has drawn a target for each of its settings. Input: the catalog entry. Return: the document body. */
async function mountPage(page: PageEntry) {
  clearMocks(); // here, not after a test: the page's listeners unregister asynchronously after it unmounts and still need the previous mock
  mockWindows("main");
  mockIPC(answer, { shouldMockEvents: true });
  const root = document.body.appendChild(document.createElement("div"));
  stop = render(() => [createComponent(TitleBar, { title: "Cue", page: page.label, feature: page.feature?.key, featureRow: page.feature?.row }), createComponent(page.component, {})] as never, root);
  await vi.waitFor(
    () => {
      expect(document.body.textContent?.length).toBeGreaterThan(0);
      for (const s of SETTINGS.filter((x) => x.page === page.id)) {
        expect(document.body.querySelector(`[data-setting="${s.id}"]`), `${page.id}: ${s.id}`).not.toBeNull();
      }
    },
    { timeout: READY_MS },
  );
  return document.body;
}

describe("page catalog", () => {
  it("has unique page ids and a search page for every search entry", () => {
    const ids = PAGES.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const s of SETTINGS) expect(ids, `setting ${s.id}`).toContain(s.page);
  });

  it("names a real search entry for every feature switch", () => {
    for (const p of PAGES) {
      const row = "feature" in p && p.feature && "row" in p.feature ? p.feature.row : undefined;
      if (row) expect(SETTINGS.map((s) => s.id)).toContain(row);
    }
  });

  it.each(PAGES.map((p) => [p.id, p] as const))("renders the %s page with a target for each of its settings", async (_id, page) => {
    await mountPage(page);
  }, READY_MS + 2_000);
});

describe("PageId", () => {
  it("is a union of literals: a name that is not a page does not compile", () => {
    const widest: string extends PageId ? "widened" : "literal" = "literal";
    expect(widest).toBe("literal");
    // @ts-expect-error "nope" is not a page id
    const bad: PageId = "nope";
    // @ts-expect-error go takes a page id
    const refused = () => go("nope");
    expect([bad, refused]).toHaveLength(2);
  });
});

describe("parsePage", () => {
  it("accepts a page id and refuses anything else", () => {
    expect(parsePage("about")).toBe("about");
    for (const bad of ["nope", "", null, undefined, 3, {}, "ABOUT"]) expect(parsePage(bad)).toBeUndefined();
  });
});
