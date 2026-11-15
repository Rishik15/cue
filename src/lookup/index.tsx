/**
 * File: lookup/index.tsx
 * Purpose: Entry of the dictionary window. The window is transparent (the surface draws its own card and shadow), so the page paints no background. Inter is loaded before the first render so the card is measured in its real font.
 * Contents: Failed — a page error closes the window; start.
 */
import "../styles.css";
import { ErrorBoundary } from "solid-js";
import { render } from "solid-js/web";
import { initFocusModality } from "../focus";
import { LookupApp } from "./LookupApp";
import { closeLookup } from "./useLookup";

document.body.style.background = "transparent";
initFocusModality();

/** A page error closes the window instead of leaving a broken card on top of everything. */
function Failed() {
  void closeLookup();
  return null;
}

/** Loads the font, then renders the page. */
async function start() {
  await document.fonts.load('400 13px "Inter Variable"').catch(() => undefined);
  render(
    () => (
      <ErrorBoundary fallback={() => <Failed />}>
        <LookupApp />
      </ErrorBoundary>
    ),
    document.getElementById("root") as HTMLElement,
  );
}

void start();
