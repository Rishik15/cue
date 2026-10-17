/**
 * Purpose: The setup pages of the walkthrough, one per feature: what the page says, the shortcut it lets the user change, and the download it offers (if any).
 * Contents: PageDef — one setup page; PAGES — the pages in order (they follow the welcome and the feature list); STEP_COUNT — every step of the walkthrough.
 */
export type PageDef = { tag: string; title: string; text: string; shortcut: string; model?: string };

export const PAGES: PageDef[] = [
  { tag: "Dictation", title: "Talk, don't type", text: "Hold the shortcut, speak, let go. The text is typed where your cursor is.", shortcut: "dictation", model: "parakeet-v2" },
  { tag: "Transform Text", title: "Transform any selected text", text: "Select text in any app and press the shortcut to fix, convert or extract it.", shortcut: "selection" },
  { tag: "Grab Text", title: "Copy text from the screen", text: "Press the shortcut and drag over any text you can see.", shortcut: "grab" },
  { tag: "Snippets", title: "Type less", text: "Press the shortcut to search your snippets and insert one.", shortcut: "snippets" },
  { tag: "Dictionary", title: "Look up words", text: "Select a word and press the shortcut.", shortcut: "define", model: "dictionary-en" },
];

/** The welcome, the feature list, then one page per entry of `PAGES`. */
export const STEP_COUNT = 2 + PAGES.length;
