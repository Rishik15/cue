/**
 * Purpose: The remappable vim keys: their defaults and how the user's overrides are merged in.
 * Contents: DEFAULT_KEYS — action to key; Action; resolveKeys — key to action, ignoring overrides that clash.
 */
const DEFAULTS = { left: "h", down: "j", up: "k", right: "l", word: "w", back: "b", end: "e", start: "0", finish: "$", insert: "i", append: "a", below: "o", visual: "v", cut: "x", paste: "p", undo: "u", help: "?" } as const;
export type Action = keyof typeof DEFAULTS;
export const DEFAULT_KEYS: Record<string, string> = DEFAULTS;

/** Keys with a fixed meaning (including the g, d, y, c prefixes) cannot be remapped onto. */
const RESERVED = "gdycGDIAOVP^";

export function resolveKeys(overrides: Record<string, string>): Map<string, Action> {
  const keys: Record<string, string> = { ...DEFAULTS };
  for (const [action, key] of Object.entries(overrides)) {
    const clash = key.length !== 1 || RESERVED.includes(key) || Object.entries(keys).some(([a, k]) => a !== action && k === key);
    if (action in DEFAULTS && !clash) keys[action] = key;
  }
  return new Map(Object.entries(keys).map(([action, key]) => [key, action as Action]));
}
