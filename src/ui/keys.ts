/**
 * File: ui/keys.ts
 * Purpose: The one input-method guard: every Enter or Escape handler in a text context starts with it, so a key that accepts an IME candidate never also commits a chip, a draft or a window.
 * Contents: composing — whether a key belongs to an input method.
 */

/** Whether a key belongs to an input method's composition (WebView2 may report only the legacy key code 229). Input: the key event. Return: the answer. */
export const composing = (e: KeyboardEvent) => e.isComposing || e.keyCode === 229;
