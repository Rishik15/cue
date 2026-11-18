/**
 * Purpose: Contract between the Dictation page and the Rust side: every setting the page writes is read by Rust under the same key, with the same default, and every value a choice stores is one Rust understands (a name it compares (the default needs none: Rust falls back to it), or a number inside the range it clamps to).
 * Contents: sources read as text (the page, `prefs.rs`, `mode.rs`, `deliver.rs`, `mute.rs`); `reader` — the one Rust consumer of each setting; `choices` and `settings` — what the page declares; the three contract tests.
 */
import { describe, expect, it } from "vitest";
import page from "./pages/Dictation.tsx?raw";
import prefs from "../src-tauri/src/prefs.rs?raw";
import mode from "../src-tauri/src/dictation/mode.rs?raw";
import deliver from "../src-tauri/src/dictation/deliver.rs?raw";
import mute from "../src-tauri/src/dictation/mute.rs?raw";

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

/** Every line of `source` that names `"key"`, joined. Throws when none does, so a setting Rust no longer reads fails the test. */
function lines(source: string, key: string): string {
  const found = source.split("\n").filter((l) => l.includes(`"${key}"`));
  if (found.length === 0) throw new Error(`Rust never reads ${key}`);
  return found.join("\n");
}

/** The body of the Rust function that starts with `signature`. Throws when it is not found. */
function body(source: string, signature: string): string {
  const start = source.indexOf(signature);
  if (start < 0) throw new Error(`no ${signature} in the Rust sources`);
  return source.slice(start, source.indexOf("\n}\n", start));
}

/** Where Rust reads a setting: `defaults` holds its default and range, `values` the names it compares the value with. */
type Reader = { defaults: string; values: string };

/** The one Rust consumer of each setting the page writes. A setting the page adds without an entry here fails the test. */
function reader(key: string): Reader {
  const own = (source: string): Reader => ({ defaults: lines(source, key), values: lines(source, key) });
  switch (key) {
    case "dictation_mode": return { defaults: body(mode, "pub fn parse"), values: body(mode, "pub fn parse") };
    case "auto_submit": return { defaults: lines(prefs, key), values: body(deliver, "fn submit_of") };
    case "mute_while_recording": return own(mute);
    default: return own(prefs);
  }
}

/** The default Rust reads `key` with (`text("key", "x")`, `get_u64("key", 5)`, `get_bool("key", true)`, or the `_ =>` arm of `Mode::parse`). Throws when it cannot be read, so a changed reader fails instead of skipping the check. */
function rustDefault(key: string, { defaults }: Reader): unknown {
  if (key === "dictation_mode") return /_ => Mode::(\w+)/.exec(defaults)?.[1];
  const found = new RegExp(`\\("${key}",\\s*([^)]+?)\\)`).exec(defaults)?.[1];
  if (found === undefined) throw new Error(`cannot read the Rust default of ${key}`);
  return JSON.parse(found.replace(/(?<=\d)_(?=\d)/g, ""));
}

/** The range Rust clamps a numeric setting to (`.clamp(30, 3600)` or `.min(5_000)`). Throws when there is none. */
function rustRange(key: string, { defaults }: Reader): [number, number] {
  const tail = new RegExp(`"${key}",\\s*[^)]+\\)\\s*\\.(clamp|min)\\(([\\d_, ]+)\\)`).exec(defaults);
  const bounds = (tail?.[2] ?? "").split(",").map((n) => Number(n.replace(/_/g, "")));
  const [low, high] = tail?.[1] === "min" ? [0, ...bounds] : bounds;
  if (low === undefined || high === undefined || bounds.length !== (tail?.[1] === "min" ? 1 : 2)) throw new Error(`cannot read the Rust range of ${key}`);
  return [low, high];
}

describe("Dictation page and prefs.rs", () => {
  it("finds what the page declares, so the other tests are not vacuous", () => {
    expect(choices.map((c) => c.key)).toEqual(expect.arrayContaining(["mic_linger_secs", "max_record_secs", "min_record_ms", "dictation_mode", "output_action", "insert_method", "auto_submit"]));
    expect(settings.map((s) => s.key)).toEqual(expect.arrayContaining(["mic_device", "show_overlay", "remove_fillers", "vocabulary"]));
  });

  it("is read by Rust under the same key with the same default", () => {
    for (const { key, fallback } of [...choices, ...settings]) {
      const consumer = reader(key);
      expect(rustDefault(key, consumer), `default of ${key}`).toEqual(fallback);
    }
  });

  it("stores only values Rust understands, and a default the page can show", () => {
    for (const { key, table, fallback } of choices) {
      const consumer = reader(key);
      const values = Object.values(table);
      expect(values, `default of ${key} is not one of its choices`).toContain(fallback);
      const range = typeof fallback === "number" ? rustRange(key, consumer) : undefined;
      for (const value of values) {
        if (typeof value === "string" && value !== fallback) expect(consumer.values, `${key}: Rust never compares "${value}"`).toContain(`"${value}"`);
        if (typeof value === "number" && range) expect(value, `${key}: ${value} outside what Rust keeps`).toBeGreaterThanOrEqual(range[0]);
        if (typeof value === "number" && range) expect(value).toBeLessThanOrEqual(range[1]);
      }
    }
  });
});
