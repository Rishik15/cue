/**
 * Purpose: The setup pages of the walkthrough, one per feature: what the page says, the shortcut it lets the user change, and the download it offers (if any).
 * Contents: PageDef — one setup page; PAGES — the pages in order (they follow the welcome, the feature list and the overview); STEP_COUNT — every step of the walkthrough.
 */
export type PageDef = { tag: string; title: string; text: string; shortcut?: string; /** A model id, or "speech" / "text" for the one recommended for this PC. */ model?: string };

export const PAGES: PageDef[] = [
  { tag: "Dictation", title: "Talk instead of typing", text: "Hold the shortcut and speak. When you let go, your words appear wherever you were typing.", shortcut: "dictation", model: "speech" },
  { tag: "Transform Text", title: "Tidy up text where it is", text: "Select text in any app and press the shortcut. Pick an action to fix it, convert it or pull something out.", shortcut: "selection" },
  { tag: "Grab Text", title: "Copy text from anywhere on screen", text: "Press the shortcut and drag over any text you see, even text you can't select. It goes to your clipboard.", shortcut: "grab" },
  { tag: "AI Text Model", title: "Add a little AI", text: "Optional. A small model on your PC can rewrite and polish text. Nothing leaves your computer.", model: "text" },
  { tag: "Dictionary", title: "Look up a word", text: "Select a word in any app and press the shortcut for its meaning, without opening a browser.", shortcut: "define", model: "dictionary-en" },
];

/** The welcome, the feature list, the setup overview, then one page per entry of `PAGES`. */
export const STEP_COUNT = 3 + PAGES.length;
