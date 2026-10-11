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
  { id: "dictation-shortcut", page: "general", label: "Dictation Shortcut", words: "hotkey key keyboard record" },
  { id: "selection-shortcut", page: "general", label: "Selection Shortcut", words: "hotkey key keyboard rules" },
  { id: "dictation-mode", page: "general", label: "Dictation Mode", words: "hold toggle talk" },
  { id: "appearance-mode", page: "general", label: "Appearance Mode", words: "light dark system" },
  { id: "theme", page: "general", label: "Theme", words: "colors appearance catppuccin tokyo gruvbox solarized" },
  { id: "unload-models", page: "general", label: "Unload Models", words: "memory idle performance ram" },
  { id: "statistics", page: "dictation", label: "Statistics", words: "wpm words time saved activity heatmap overview" },
  { id: "mic-default", page: "dictation", label: "Use System Default", words: "microphone input" },
  { id: "mic-device", page: "dictation", label: "Input Device", words: "microphone mic" },
  { id: "mic-linger", page: "dictation", label: "Keep Mic Open", words: "microphone mic indicator" },
  { id: "mute", page: "dictation", label: "Mute Audio While Recording", words: "speakers volume sound silence" },
  { id: "max-length", page: "dictation", label: "Maximum Length", words: "recording seconds limit" },
  { id: "min-tap", page: "dictation", label: "Ignore Short Taps", words: "recording accidental" },
  { id: "overlay-show", page: "dictation", label: "Show Recording Overlay", words: "pill caption" },
  { id: "overlay-position", page: "dictation", label: "Overlay Position", words: "top bottom pill caption" },
  { id: "output-action", page: "dictation", label: "Output Action", words: "insert copy clipboard paste" },
  { id: "insert-method", page: "dictation", label: "Always Paste", words: "type paste" },
  { id: "restore-clipboard", page: "dictation", label: "Restore Clipboard", words: "paste copy" },
  { id: "context-paste", page: "dictation", label: "Context-Aware Paste", words: "capitalization space cursor" },
  { id: "auto-submit", page: "dictation", label: "Auto Submit", words: "enter send submit" },
  { id: "filler", page: "dictation", label: "Remove Filler Words", words: "um uh vocabulary" },
  { id: "vocabulary", page: "dictation", label: "Custom Words", words: "vocabulary names replace spelling" },
  { id: "speech-models", page: "models", label: "Speech Models", words: "download parakeet whisper canary language" },
  { id: "rules-list", page: "rules", label: "Rules", words: "automation text actions selection dictation" },
  { id: "remove-data", page: "about", label: "Remove All Cue Data", words: "delete reset uninstall" },
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
