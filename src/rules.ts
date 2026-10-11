/**
 * Purpose: Rules as the UI sees them: types matching `src-tauri/src/rules/model.rs`, the labels shown for each choice, and the Rust commands.
 * Contents: Rule / Condition — the stored shape; TRIGGERS / WHEN / OUTPUT / CONDITION_TYPES / KINDS — label tables; newCondition — a blank
 * condition of a type; summary — one line describing a rule; getRules / saveRules / testRules / listActions / getLog — commands.
 */
import { invoke } from "@tauri-apps/api/core";

export type Trigger = "dictation" | "selection";
export type Kind = "url" | "email" | "json" | "list";
export type Condition =
  | { type: "contains" | "starts_with" | "ends_with"; value: string }
  | { type: "matches"; pattern: string }
  | { type: "longer_than" | "shorter_than"; chars: number }
  | { type: "looks_like"; kind: Kind };
export type Rule = {
  name: string;
  enabled: boolean;
  trigger: Trigger;
  apps: string[];
  when: "all" | "any" | "none";
  conditions: Condition[];
  actions: string[];
  output: "insert" | "copy" | "preview";
  stop: boolean;
};
export type LogEntry = { at: number; trigger: Trigger; matched: string[]; app: string };
export type ActionInfo = { id: string; label: string; hint: string };
export type TestResult = { text: string; output: Rule["output"]; matched: string[] };

export const TRIGGERS: Record<string, Trigger> = { "Dictation Finished": "dictation", "Shortcut on Selection": "selection" };
export const WHEN: Record<string, Rule["when"]> = { "All of These": "all", "Any of These": "any", "None of These": "none" };
export const OUTPUT: Record<string, Rule["output"]> = { "Insert Text": "insert", "Show Preview": "preview", "Copy to Clipboard": "copy" };
export const KINDS: Record<string, Kind> = { Link: "url", Email: "email", JSON: "json", List: "list" };
export const CONDITION_TYPES: Record<string, Condition["type"]> = {
  Contains: "contains",
  "Starts With": "starts_with",
  "Ends With": "ends_with",
  "Matches Pattern": "matches",
  "Longer Than": "longer_than",
  "Shorter Than": "shorter_than",
  "Looks Like": "looks_like",
};

/** Looks up the label for a stored value in a label table. */
export const labelOf = <T extends string>(table: Record<string, T>, value: T) => Object.keys(table).find((k) => table[k] === value) ?? "";

export function newCondition(type: Condition["type"]): Condition {
  if (type === "matches") return { type, pattern: "" };
  if (type === "longer_than" || type === "shorter_than") return { type, chars: 100 };
  if (type === "looks_like") return { type, kind: "url" };
  return { type, value: "" };
}

export const newRule = (): Rule => ({
  name: "New Rule",
  enabled: true,
  trigger: "dictation",
  apps: [],
  when: "all",
  conditions: [],
  actions: [],
  output: "insert",
  stop: false,
});

export const summary = (r: Rule, actions: ActionInfo[]) =>
  [labelOf(TRIGGERS, r.trigger), ...r.actions.map((id) => actions.find((a) => a.id === id)?.label ?? id)].join(" · ");

export const getRules = () => invoke<Rule[]>("get_rules").then((r) => (Array.isArray(r) ? r : []));
export const saveRules = (rules: Rule[]) => invoke("save_rules", { rules });
export const listActions = () => invoke<ActionInfo[]>("list_actions").then((a) => (Array.isArray(a) ? a : []));
export const testRules = (rules: Rule[], trigger: Trigger, text: string, app: string) =>
  invoke<TestResult>("test_rules", { rules, trigger, text, app });
export const getLog = () => invoke<LogEntry[]>("rule_log").then((l) => (Array.isArray(l) ? l : []));
