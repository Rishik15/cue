/**
 * Purpose: Text page: the rules that act on selected text (cleanup, links, correct and rewrite with the local model).
 * Contents: Text — the shared rules host for the selection trigger.
 */
import { Rules } from "./Rules";

export const Text = () => <Rules trigger="selection" />;
