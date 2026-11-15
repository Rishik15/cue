/**
 * File: lookup/useLookup.ts
 * Purpose: The dictionary window page's connection to Rust and its state. It listens for the one `dict-open` event (registered before it says it is ready, so the event cannot be missed), checks it, paints the saved theme and starts either the card for a word or the search window; keys, typing and clicks go through the pure rules in `nav.ts`, and each effect is one command (the window that calls names its lookup to Rust, so no number is sent): search a prefix, open a word's entry, place the window, close. A reply to a search the user has typed past, or to an entry the user has left, is dropped.
 * Contents: closeLookup — close that cannot fail; listenOpen — the open event, then ready; useMatches — searching a prefix; shown / recentOf / usable — what the results list; readEntry — a word's entry or why it could not be read; skipped / fromField — which keys the rules see; useResults — the search window's state and handlers; useLookup — the signals and actions the page uses.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { paintSaved } from "../themes";
import { fold, press, pick, start, typed, type Effect, type Nav } from "./nav";
import { asEntry, asHits, asOpen, asPlacement, type Entry, type Hit, type Open, type Placement } from "./payload";

/** Closes the window through Rust at once (the page holds no other way to close it; a hidden window that never shows is closed by Rust's watchdog). Return: a promise that never rejects. */
export const closeLookup = () => invoke("dict_close").catch(() => undefined);

/** Listens for the open event, then tells Rust the page is ready, so the event cannot go out before anyone listens; a failure closes the window. Input: what to do with the payload. Return: the function that stops listening. */
function listenOpen(receive: (raw: unknown) => void): () => void {
  let off: (() => void) | undefined;
  let gone = false;
  listen("dict-open", (e) => receive(e.payload)).then(
    (unlisten) => {
      if (gone) return unlisten();
      off = unlisten;
      invoke("dict_ready").catch(() => void closeLookup());
    },
    () => void closeLookup(),
  );
  /** Stops listening, now or once the listener is registered. */
  return () => {
    gone = true;
    off?.();
  };
}

/** What a query found: the matches with the query they answer, or why there are none. */
type Found = { query: string; found: Hit[]; problem: string };

/** Searching a typed prefix through Rust. Input: the open payload. Return: the latest answer and `search`, which asks (an answer for a query the user has typed past is dropped). */
function useMatches(open: () => Open | null) {
  const [hits, setHits] = createSignal<Found>({ query: "", found: [], problem: "" });
  let latest = 0;
  /** Searches the prefix `query`. Input: the query. Return: a promise that settles once the answer is stored or dropped. */
  const search = async (query: string) => {
    const mine = ++latest;
    if (!open() || !query.trim()) return setHits({ query, found: [], problem: "" });
    const answer = await invoke("dict_search", { query }).then(
      (raw) => ({ found: asHits(raw), problem: "" }),
      (e) => ({ found: [] as Hit[], problem: String(e) }),
    );
    if (mine === latest) setHits({ query, ...answer });
  };
  return { hits, search };
}

/** The matches to show for the query on screen: none when the field is empty; the reply when it is for this query; while that reply is out, the previous reply's matches this query still starts (the server's own folding), so typing on never blanks the list and a word that no longer matches never stays. Input: whether there is a query, the query and the latest reply. Return: the matches. */
function shown(querying: boolean, query: string, reply: Found): Hit[] {
  if (!querying) return [];
  if (reply.query === query) return reply.found;
  const prefix = fold(query);
  return reply.found.filter((h) => h.key.startsWith(prefix));
}

/** The recent words of an open search window, none for a card or before the open event. Input: the payload. Return: the words. */
const recentOf = (o: Open | null) => (o?.mode === "search" ? o.recent : []);

/** Whether a window has anything to open: a search window with a pack installed (without one no row is shown, so none may be opened). Input: the payload. Return: the answer. */
const usable = (o: Open | null) => o?.mode === "search" && o.pack;

/** Reads a word's entry through Rust. Input: the word. Return: the entry (null when no pack has the word) and, when the call failed, why (empty otherwise). */
const readEntry = (word: string) =>
  invoke("dict_define", { word }).then(
    (raw) => ({ found: asEntry(raw), why: "" }),
    (e: unknown) => ({ found: null, why: String(e) || "The entry could not be read." }),
  );

/** Whether a key is the input method's or a shortcut's, never the rules'. Input: the event. Return: the answer. */
const skipped = (e: KeyboardEvent) => e.isComposing || e.keyCode === 229 || e.ctrlKey || e.altKey || e.metaKey;

/** Whether a key may drive the results: Escape from anywhere in the window, the rest only from the search field, so a focused button (the no-pack Download) keeps its own Enter and Space. Input: the event. Return: the answer. */
const fromField = (e: KeyboardEvent) => e.key === "Escape" || (e.target instanceof HTMLInputElement && e.target.getAttribute("role") === "combobox");

/** The search window's state and handlers: the query and highlight (the pure rules in `nav.ts`), the matches, the entry on show, and what typing, keys and clicks do. Input: the open payload. Return: the signals and handlers. */
function useResults(open: () => Open | null) {
  const [nav, setNav] = createSignal<Nav>(start());
  const { hits, search } = useMatches(open);
  const [entry, setEntry] = createSignal<Entry | null>(null);
  const [failed, setFailed] = createSignal("");
  let defining = 0;
  /** The recent words of an empty search window. */
  const recent = () => recentOf(open());
  /** Whether the field has a query to search. */
  const querying = () => nav().query.trim() !== "";
  /** Whether the reply on hand is for the query on screen (a newer search is still out otherwise). */
  const answered = () => hits().query === nav().query;
  /** The matches on show: the one list rows, keys, highlight, preview and the result count read (`shown`). */
  const visible = createMemo(() => shown(querying(), nav().query, hits()));
  /** The words keys and clicks may open: the matches on show, else the recent words; none without a pack (`usable`). */
  const rows = createMemo(() => (!usable(open()) ? [] : querying() ? visible().map((h) => h.key) : recent().map((r) => r.word)));
  /** Why the search for the query on screen failed, empty when it did not (or its reply is still out). */
  const problem = () => (answered() ? hits().problem : "");
  /** Whether a reply says the query on screen has no match; never before the reply. */
  const noMatch = () => querying() && answered() && hits().found.length === 0 && !hits().problem;

  /** Opens word `word`'s entry (only a listed row gets here, and rows are listed only in an open search window with a pack); a word with no entry returns to the results, a failed call (a pack that cannot be read, which Rust also reports) stays on the entry page and says why. Input: the word. Return: a promise that settles once the entry or the failure is shown, or dropped (a newer request, or the user left the entry). */
  const define = async (word: string) => {
    const mine = ++defining;
    setEntry(null);
    setFailed("");
    const reply = await readEntry(word);
    if (mine !== defining || nav().stage !== "entry") return;
    if (reply.why) setFailed(reply.why);
    else if (reply.found) setEntry(reply.found);
    else setNav((n) => ({ ...n, stage: "typing" }));
  };

  /** Runs what a key or click decided. Input: the effect. Return: none. */
  const run = (effect: Effect) => {
    if (effect.kind === "define") void define(effect.word);
    if (effect.kind === "close") void closeLookup();
  };

  /** A key in the window: the pure rules decide (only keys `fromField` lets through); a key they own does not reach the field. Input: the key event. Return: none. */
  const onKey = (e: KeyboardEvent) => {
    if (skipped(e) || !fromField(e) || open()?.mode !== "search") return;
    const next = press(nav(), e.key, rows());
    if (!next.handled) return;
    e.preventDefault();
    setNav(next.nav);
    run(next.effect);
  };

  /** A row clicked. Input: its index. Return: none. */
  const onPick = (row: number) => {
    const next = pick(nav(), row, rows());
    setNav(next.nav);
    run(next.effect);
  };

  return {
    nav, visible, entry, setEntry, failed, recent, rows, querying, answered, noMatch, problem, onKey, onPick,
    /** The field's text changed. Input: the new text. Return: none. */
    onInput: (query: string) => {
      setNav(typed(query));
      void search(query);
    },
    /** Moves the highlight to a row under the pointer. Input: the row. Return: none. */
    onPoint: (row: number) => setNav((n) => (n.sel === row || n.stage !== "typing" ? n : { ...n, sel: row })),
  };
}

/** The page's state and actions: the open payload, where Rust put the window, and the search window's state (`useResults`); it listens for the open event and the keys for as long as the page lives. Return: the signals and handlers the components read. */
export function useLookup() {
  const [open, setOpen] = createSignal<Open | null>(null);
  const [placement, setPlacement] = createSignal<Placement | null>(null);
  const results = useResults(open);

  /** Asks Rust for the window at a size (dip) and keeps where it went; a placement it refuses closes the window. Input: the size. Return: a promise that settles once the placement is kept or refused. */
  const place = async (w: number, h: number) => {
    if (!open()) return;
    const got = await invoke("dict_place", { w, h }).then(asPlacement, () => null);
    if (got) setPlacement(got);
    else void closeLookup();
  };

  /** Takes the open event: checks it, paints the theme and starts the mode it names. Input: the raw payload. Return: none. */
  const receive = (raw: unknown) => {
    const next = asOpen(raw);
    if (!next) return void closeLookup();
    document.documentElement.dataset.kind = paintSaved(next.theme);
    if (next.mode === "card") results.setEntry(next.entry);
    setOpen(next);
  };

  onMount(() => {
    const stop = listenOpen(receive);
    window.addEventListener("keydown", results.onKey);
    onCleanup(() => {
      stop();
      window.removeEventListener("keydown", results.onKey);
    });
  });

  return {
    ...results,
    open,
    placement,
    place,
    /** The no-pack button: Rust closes the window and opens the Dictionary page of the settings. */
    download: () => void invoke("dict_download").catch(() => void closeLookup()),
  };
}
