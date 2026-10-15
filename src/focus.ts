/**
 * Purpose: Show the keyboard focus ring only after Tab, never after a mouse click or arrow keys (a list moved with arrows highlights its row, not the box that holds the keyboard).
 * Contents: initFocusModality — tracks the last input type on <html data-modality>; styles.css keys the ring off it.
 */
export function initFocusModality() {
  const root = document.documentElement;
  window.addEventListener("keydown", (e) => { if (e.key === "Tab") root.dataset.modality = "keyboard"; }, true);
  window.addEventListener("pointerdown", () => { root.dataset.modality = "pointer"; }, true);
}
