/**
 * Purpose: Show the keyboard focus ring only after keyboard use, never after a mouse click.
 * Contents: initFocusModality — tracks the last input type on <html data-modality>; styles.css keys the ring off it.
 */
export function initFocusModality() {
  const root = document.documentElement;
  window.addEventListener("keydown", (e) => { if (e.key === "Tab" || e.key.startsWith("Arrow")) root.dataset.modality = "keyboard"; }, true);
  window.addEventListener("pointerdown", () => { root.dataset.modality = "pointer"; }, true);
}
