/**
 * Purpose: Basic vim keys for the note editor: Normal, Insert and Visual modes over CodeMirror's own commands.
 * Contents: VimMode / VimHooks — what the page hears from it; vim — the extension (nothing when switched off).
 */
import { Prec, type Extension } from "@codemirror/state";
import { EditorView, ViewPlugin } from "@codemirror/view";
import { keyHandler } from "./commands";
import { resolveKeys } from "./keys";
import { modeField, type VimMode } from "./mode";

export type { VimMode } from "./mode";
/** What the editor tells the page: the mode on every change, and a request for the help page. */
export type VimHooks = { onMode: (mode: VimMode | null) => void; onHelp: () => void };

export function vim(enabled: boolean, keys: Record<string, string>, hooks: VimHooks): Extension {
  if (!enabled) return [];
  const handle = keyHandler(resolveKeys(keys), hooks);
  // A plugin is rebuilt with every fresh state, so it reports Normal again when another note opens.
  const report = ViewPlugin.define((view) => {
    hooks.onMode(view.state.field(modeField));
    return { update: (u) => { if (u.state.field(modeField) !== u.startState.field(modeField)) hooks.onMode(u.state.field(modeField)); } };
  });
  return [modeField, report, Prec.highest(EditorView.domEventHandlers({ keydown: (e, view) => handle(view, e) }))];
}
