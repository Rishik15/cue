/**
 * Purpose: Contract between the Dictation page and the Rust side: every setting the page writes is read by Rust under the same key, with the same default, and every value a choice stores is one Rust understands (a name it compares (the default needs none: Rust falls back to it), or a number inside the range it clamps to).
 * Contents: sources read as text (the page, `prefs.rs`, `mode.rs`, `deliver.rs`, `mute.rs`, `dictation/mod.rs`); `choices` and `settings` — what the page declares; the three contract tests.
 */
import { describe, expect, it } from "vitest";
import page from "./pages/Dictation.tsx?raw";
import prefs from "../src-tauri/src/prefs.rs?raw";
import mode from "../src-tauri/src/dictation/mode.rs?raw";
import deliver from "../src-tauri/src/dictation/deliver.rs?raw";
import mute from "../src-tauri/src/dictation/mute.rs?raw";
import glue from "../src-tauri/src/dictation/mod.rs?raw";

const rust = [prefs, mode, deliver, mute, glue].join("\n");

type Table = Record<string, string | number>;
type Choice = { key: string; table: Table; fallback: unknown };

/** Reads a literal from the page source (`{ Off: 0, "5 Seconds": 5 }`, `"Hold"`, `1200`, `true`): bare keys are quoted, then it is JSON. */
const literal = (text: string): unknown => JSON.parse(text.replace(/([{,]\s*)([A-Za-z_]\w*)\s*:/g, '$1"$2":'));

/** The `Record<string, ...>` tables the page declares, by name. */
const tables: Record<string, Table> = Object.fromEntries(
  [...page.matchAll(/const (\w+): Record<string, \w+> = (\{[^}]*\});/g)].map((m): [string, Table] => [m[1] ?? "", literal(m[2] ?? "{}") as Table]),
);

/** Every `useChoice("key", TABLE, default)` of the page. */
const choices: Choice[] = [...page.matchAll(/useChoice\("(\w+)", (\w+), ([^)]+)\)/g)].map((m) => ({ key: m[1] ?? "", table: tables[m[2] ?? ""] ?? {}, fallback: literal(m[3] ?? "") }));

/** Every plain `useSetting("key", default)` of the page. */
const settings = [...page.matchAll(/useSetting\("(\w+)", ([^)]+)\)/g)].map((m) => ({ key: m[1] ?? "", fallback: literal(m[2] ?? "") }));

/** The default Rust reads `key` with (`text("key", "x")`, `get_u64("key", 5)`, `get_bool("key", true)`), or undefined when it has none or computes it. */
function rustDefault(key: string): unknown {
  const found = new RegExp(`\\("${key}",\\s*([^)]+?)\\)`).exec(rust)?.[1];
  try {
    return found === undefined ? undefined : JSON.parse(found);
  } catch {
    return undefined;
  }
}

/** The range Rust clamps a numeric setting to (`.clamp(30, 3600)` or `.min(5_000)`), or undefined. */
function rustRange(key: string): [number, number] | undefined {
  const tail = new RegExp(`"${key}",\\s*[^)]+\\)\\s*\\.(clamp|min)\\(([\\d_, ]+)\\)`).exec(rust);
  const bounds = (tail?.[2] ?? "").split(",").map((n) => Number(n.replace(/_/g, "")));
  if (tail?.[1] === "min") return [0, bounds[0] ?? 0];
  return bounds.length === 2 ? [bounds[0] ?? 0, bounds[1] ?? 0] : undefined;
}

describe("Dictation page and prefs.rs", () => {
  it("finds what the page declares, so the other tests are not vacuous", () => {
    expect(choices.map((c) => c.key)).toEqual(expect.arrayContaining(["mic_linger_secs", "max_record_secs", "min_record_ms", "dictation_mode", "output_action", "insert_method", "auto_submit"]));
    expect(settings.map((s) => s.key)).toEqual(expect.arrayContaining(["mic_device", "show_overlay", "remove_fillers", "vocabulary"]));
  });

  it("is read by Rust under the same key with the same default", () => {
    for (const { key, fallback } of [...choices, ...settings]) {
      expect(rust, `Rust never reads ${key}`).toContain(`"${key}"`);
      const theirs = rustDefault(key);
      if (theirs !== undefined) expect(theirs, `default of ${key}`).toEqual(fallback);
    }
  });

  it("stores only values Rust understands, and a default the page can show", () => {
    for (const { key, table, fallback } of choices) {
      const values = Object.values(table);
      expect(values, `default of ${key} is not one of its choices`).toContain(fallback);
      const range = rustRange(key);
      for (const value of values) {
        if (typeof value === "string" && value !== fallback) expect(rust, `${key}: Rust never compares "${value}"`).toContain(`"${value}"`);
        else if (range) expect(value, `${key}: ${value} outside what Rust keeps`).toBeGreaterThanOrEqual(range[0]);
        if (typeof value === "number" && range) expect(value).toBeLessThanOrEqual(range[1]);
      }
    }
  });
});
