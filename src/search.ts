/**
 * Purpose: Search over the actual settings, not only the pages: every setting a user can change, with the page it lives on, and the matching
 * that the sidebar search uses.
 * Contents: SETTINGS — the index (an id per setting, the same id is on its row as `data-setting`); SettingId — the union of ids, so a row
 * cannot use an id that is not indexed; search — pages and settings matching a query, grouped by page; parts — a label split for highlighting;
 * openSetting — go to a setting's page, scroll to its row and flash it.
 */
import { go } from "./nav";

type Entry = { id: string; page: string; label: string; words?: string };

export const SETTINGS = [
  { id: "launch-login", page: "general", label: "Launch at Login", words: "startup autostart tray boot" },
  { id: "open-on-launch", page: "general", label: "Open Settings on Launch", words: "window start" },
  { id: "dictation-shortcut", page: "shortcuts", label: "Dictation Shortcut", words: "hotkey key keyboard record" },
  { id: "selection-shortcut", page: "shortcuts", label: "Transform Text Shortcut", words: "hotkey key keyboard rules selection" },
  { id: "grab-shortcut", page: "shortcuts", label: "Grab Text Shortcut", words: "hotkey key keyboard ocr screen capture" },
  { id: "define-shortcut", page: "shortcuts", label: "Dictionary Shortcut", words: "hotkey key keyboard define word meaning lookup" },
  { id: "ocr-languages", page: "capture", label: "Recognition Languages", words: "ocr language install windows text recognition" },
  { id: "snippets-shortcut", page: "shortcuts", label: "Snippets Shortcut", words: "hotkey key keyboard saved text" },
  { id: "snippets-expand", page: "snippets", label: "Expand Snippets While Typing", words: "trigger text expander abbreviation shortcode type replace" },
  { id: "snippets-undo", page: "snippets", label: "Undo With Backspace", words: "backspace restore take back trigger" },
  { id: "snippets-skip", page: "snippets", label: "Don't Expand In", words: "programs apps exclude ignore skip" },
  { id: "dictation-mode", page: "dictation", label: "Shortcut Behavior", words: "hold toggle talk long session lecture short phrase" },
  { id: "appearance-mode", page: "general", label: "Color Mode", words: "light dark system" },
  { id: "theme", page: "general", label: "Theme", words: "colors appearance catppuccin tokyo gruvbox solarized" },
  { id: "unload-models", page: "general", label: "Unload Models When Idle", words: "memory idle performance ram" },
  { id: "statistics", page: "dictation", label: "Statistics", words: "wpm words time saved activity heatmap overview" },
    { id: "mic-device", page: "dictation", label: "Microphone", words: "input device default mic" },
  { id: "mic-linger", page: "dictation", label: "Keep Microphone Open", words: "microphone mic indicator" },
  { id: "mute", page: "dictation", label: "Mute While Recording", words: "speakers volume sound silence" },
  { id: "max-length", page: "dictation", label: "Maximum Recording Length", words: "recording seconds limit" },
  { id: "min-tap", page: "dictation", label: "Ignore Short Taps", words: "recording accidental" },
  { id: "overlay-show", page: "dictation", label: "Recording Overlay", words: "pill caption" },
  { id: "overlay-position", page: "dictation", label: "Overlay Position", words: "top bottom pill caption" },
  { id: "output-action", page: "dictation", label: "Send Text To", words: "insert copy clipboard paste both output" },
  { id: "insert-method", page: "dictation", label: "Insert By", words: "type paste typing pasting always" },
  { id: "restore-clipboard", page: "dictation", label: "Restore Clipboard", words: "paste copy" },
  { id: "context-paste", page: "dictation", label: "Match Surrounding Text", words: "capitalization space cursor" },
  { id: "auto-submit", page: "dictation", label: "Submit After Inserting", words: "enter send submit" },
  { id: "filler", page: "dictation", label: "Remove Filler Words", words: "um uh vocabulary" },
  { id: "vocabulary", page: "dictation", label: "Custom Words", words: "vocabulary names replace spelling" },
  { id: "speech-models", page: "models", label: "Speech Models", words: "download parakeet whisper canary language" },
  { id: "text-models", page: "models", label: "Text Model", words: "correct rewrite shorter formal friendly qwen download ai" },
  { id: "dictionaries", page: "models", label: "Dictionaries", words: "dictionary word meaning definition language download english wordnet" },
  { id: "saved-words", page: "words", label: "Saved Words", words: "vocabulary dictionary words saved looked up define" },
  { id: "dictation-rules", page: "dictation", label: "Inserted Text Actions", words: "automation text actions after finish" },
  { id: "text-rules", page: "text", label: "Transform Text Actions", words: "transform automation actions selection correct rewrite shorter formal links" },
  { id: "snippets-list", page: "snippets", label: "Snippets", words: "saved text paste import export keyword templates" },
  { id: "github", page: "about", label: "Source Code", words: "star repo repository source code" },
  { id: "feedback", page: "about", label: "Feedback", words: "comments suggestion github" },
  { id: "report-issue", page: "about", label: "Bugs", words: "bug problem github" },
  { id: "remove-data", page: "about", label: "Remove All Cue Data", words: "delete reset uninstall" },
  { id: "crash-log", page: "about", label: "Problem Log", words: "crash error bug log recovered" },
  { id: "dictionary-source", page: "about", label: "Dictionary Data", words: "wordnet license attribution source cc by princeton" },
  { id: "license", page: "about", label: "License", words: "mit version" },
] as const satisfies readonly Entry[];

export type SettingId = (typeof SETTINGS)[number]["id"];
export type Group<P> = { page: P; pageHit: boolean; items: readonly Entry[] };

const matches = (text: string, q: string) => text.toLowerCase().includes(q);

/** Pages whose name matches, and settings whose name or keywords match, grouped under their page in page order. */
export function search<P extends { id: string; label: string }>(query: string, pages: P[]): Group<P>[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  return pages
    .map((page) => ({
      page,
      pageHit: matches(page.label, q),
      items: SETTINGS.filter((s) => s.page === page.id && (matches(s.label, q) || matches(s.words ?? "", q))),
    }))
    .filter((g) => g.pageHit || g.items.length > 0);
}

/** The first match of `query` in `text` as separate pieces, so the match can be highlighted. */
export function parts(text: string, query: string): { text: string; hit: boolean }[] {
  const q = query.trim().toLowerCase();
  const at = q ? text.toLowerCase().indexOf(q) : -1;
  if (at < 0) return [{ text, hit: false }];
  return [
    { text: text.slice(0, at), hit: false },
    { text: text.slice(at, at + q.length), hit: true },
    { text: text.slice(at + q.length), hit: false },
  ].filter((p) => p.text);
}

/** Opens the setting's page and scrolls its row to the middle of the page with a short flash (`data-flash`, see styles.css).
 *  Only the page scrolls: `scrollIntoView` would also move the window's own hidden scroll position, which drags the whole app up. */
export function openSetting(id: string, page: string) {
  go(page);
  // Two frames: the page renders, then it has its final height.
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      const row = document.querySelector(`[data-setting="${id}"]`);
      if (!row) return;
      const page = row.closest("main");
      if (page) {
        const offset = row.getBoundingClientRect().top - page.getBoundingClientRect().top;
        page.scrollTo({ top: page.scrollTop + offset - (page.clientHeight - row.clientHeight) / 2, behavior: "smooth" });
      }
      row.setAttribute("data-flash", "");
      setTimeout(() => row.removeAttribute("data-flash"), 1400);
    }),
  );
}
