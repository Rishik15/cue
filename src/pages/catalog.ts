/**
 * File: pages/catalog.ts
 * Purpose: The one table of settings pages. The sidebar, the header title, the page the window renders, the header's feature switch and the setting search all read it, so a new page is one entry here (plus its rows in `search.ts`).
 * Contents: PageEntry — one page; PAGES — the pages in sidebar order (General is in the main bundle, the others load when opened, which keeps their code out of the window's startup; the Kit page exists only in development); PageId — the union of page ids; parsePage — checks a page name that arrives from outside; pageById — the entry of a page.
 */
import { LayoutGrid } from "lucide-solid";
import { lazy, type Component } from "solid-js";
import { SETTING } from "../snippets";
import { Ball, Book, Chip, Gear, Keys, Mic, Note, Snippet, Text } from "../ui/glyphs";
import type { Glyph } from "../ui/NavIcon";
import { General } from "./General";

/** One settings page. `section` starts a new sidebar group (a gap above it). `feature` is the on/off setting the header shows for a page that is a feature (the shortcut names drive these keys: `dictation_enabled`...); `row` is the id of the search entry that switch answers to (it carries that `data-setting`). */
export type PageEntry = { id: string; label: string; icon: Glyph; component: Component; section?: boolean; feature?: { key: string; row?: string } };

/** A page loaded on first use. Input: the loader of its module and the name of the export. Return: the page component. */
const later = <K extends string>(load: () => Promise<Record<K, Component>>, name: K) => lazy(() => load().then((m) => ({ default: m[name] })));

/** The development-only Kit page, kept as its own literal so its id stays a literal type. */
const KIT = { id: "kit", label: "Kit", icon: LayoutGrid, component: later(() => import("./Kit"), "Kit") } as const;

export const PAGES = [
  { id: "general", label: "General", icon: Gear, component: General },
  { id: "shortcuts", label: "Shortcuts", icon: Keys, component: later(() => import("./Shortcuts"), "Shortcuts") },
  { id: "models", label: "Models", icon: Chip, component: later(() => import("./Models"), "Models") },
  { id: "dictation", label: "Dictation", icon: Mic, component: later(() => import("./Dictation"), "Dictation"), section: true, feature: { key: "dictation_enabled" } },
  { id: "text", label: "Text", icon: Text, component: later(() => import("./Text"), "Text"), feature: { key: "selection_enabled" } },
  { id: "snippets", label: "Snippets", icon: Snippet, component: later(() => import("./Snippets"), "Snippets"), feature: { key: SETTING.expand.key, row: "snippets-expand" } },
  { id: "notes", label: "Notes", icon: Note, component: later(() => import("./Notes"), "Notes"), feature: { key: "notes_enabled" } },
  { id: "words", label: "Dictionary", icon: Book, component: later(() => import("./Words"), "Words"), feature: { key: "define_enabled" } },
  { id: "about", label: "About", icon: Ball, component: later(() => import("./About"), "About"), section: true },
  // Dev only: the dynamic import keeps the Kit page out of the release bundle.
  ...(import.meta.env.DEV ? [KIT] : []),
] as const satisfies readonly PageEntry[];

export type PageId = (typeof PAGES)[number]["id"];

/** The page named `id`, or undefined for a name that is not a page. Input: any value (a payload from Rust). Return: the page id, so unknown names are refused at the door. */
export const parsePage = (id: unknown): PageId | undefined => PAGES.find((p) => p.id === id)?.id;

/** The entry of a page. Input: its id. Return: the entry. */
export const pageById = (id: PageId): PageEntry => PAGES.find((p) => p.id === id) ?? PAGES[0];
