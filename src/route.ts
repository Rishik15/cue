/**
 * File: route.ts
 * Purpose: The door for a page name that arrives from outside the window (Rust's `cue-goto` event and `take_start_page`): a known page opens, an unknown name is refused and reported, so the window never sits on a page it cannot draw.
 * Contents: goTo — open the page a name stands for.
 */
import { go } from "./nav";
import { parsePage } from "./pages/catalog";
import { showToast } from "./ui/Toast";

/** Opens the page `name` stands for, or tells the user and stays where it is when the name is not a page. Input: whatever arrived. */
export function goTo(name: unknown) {
  const page = parsePage(name);
  if (page) go(page);
  else showToast(`Cue could not open the page "${String(name)}".`);
}
