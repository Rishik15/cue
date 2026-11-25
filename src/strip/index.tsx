/**
 * File: strip/index.tsx
 * Purpose: Entry of the Transform Text window. The window is transparent (the card draws its own surface and shadow), so the page paints no background. Inter is loaded before the first render so the card is measured in its real font; a page error closes the window.
 * Contents: the boot call.
 */
import "../styles.css";
import { bootWindow } from "../ui/bootWindow";
import { StripApp } from "./StripApp";
import { closeStrip } from "./useStrip";

void bootWindow({ App: StripApp, close: closeStrip, transparent: true, fonts: [document.fonts.load('400 13px "Inter Variable"')] });
