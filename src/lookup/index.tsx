/**
 * File: lookup/index.tsx
 * Purpose: Entry of the dictionary window. The window is transparent (the surface draws its own card and shadow), so the page paints no background. Inter is loaded before the first render so the card is measured in its real font; a page error closes the window.
 * Contents: the boot call.
 */
import "../styles.css";
import { bootWindow } from "../ui/bootWindow";
import { LookupApp } from "./LookupApp";
import { closeLookup } from "./useLookup";

void bootWindow({ App: LookupApp, close: closeLookup, transparent: true, fonts: [document.fonts.load('400 13px "Inter Variable"')] });
