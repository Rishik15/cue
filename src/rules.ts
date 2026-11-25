/**
 * File: rules.ts
 * Purpose: Rules as the UI sees them: types matching `src-tauri/src/rules/model.rs`, the labels shown for each choice, and the Rust commands, whose replies are parsed before they are trusted.
 * Contents: Rule / Condition — the stored shape; TRIGGERS / WHEN / OUTPUT / CONDITION_TYPES / KINDS — label tables; newCondition — a blank
 * condition of a type; summary — one line naming a rule's actions (the page it is on already says when it runs); parseRules / parseActions / parseLog — reply checks;
 * getRules / saveRules / testRules / listActions / getLog — commands (a malformed reply rejects).
 */
import { invoke } from "@tauri-apps/api/core";
import { isBool, isNum, isRecord, isStr, listOf, oneOf, read, shaped, strings } from "./ipc";

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
export type ActionInfo = { id: string; label: string; hint: string; model: boolean; dictation: boolean };
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

/** A blank condition of a type, with the fields Rust expects. Input: the type. Return: the condition. */
export function newCondition(type: Condition["type"]): Condition {
  if (type === "matches") return { type, pattern: "" };
  if (type === "longer_than" || type === "shorter_than") return { type, chars: 100 };
  if (type === "looks_like") return { type, kind: "url" };
  return { type, value: "" };
}

/** A new, empty rule as the editor starts it. Return: the rule. */
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

/** One line naming a rule's actions by label (the id for one that no longer exists). Input: the rule and the known actions. Return: the line. */
export const summary = (r: Rule, actions: ActionInfo[]) =>
  r.actions.map((id) => actions.find((a) => a.id === id)?.label ?? id).join(" · ");

/** Whether a value is a condition: its fields are checked by its `type`. Input: the value. Return: the check. */
function isCondition(c: unknown): c is Condition {
  if (!isRecord(c)) return false;
  switch (c.type) {
    case "contains":
    case "starts_with":
    case "ends_with":
      return isStr(c.value);
    case "matches":
      return isStr(c.pattern);
    case "longer_than":
    case "shorter_than":
      return isNum(c.chars);
    case "looks_like":
      return oneOf(KINDS, c.kind);
    default:
      return false;
  }
}

/** Whether a value is a stored rule. Input: the value. Return: the check. */
const isRule = (v: unknown): v is Rule =>
  shaped<Rule>(v, {
    name: isStr, enabled: isBool, trigger: (t) => oneOf(TRIGGERS, t), apps: strings, when: (w) => oneOf(WHEN, w),
    conditions: listOf(isCondition), actions: strings, output: (o) => oneOf(OUTPUT, o), stop: isBool,
  });
/** Whether a value is an action the rules can run. Input: the value. Return: the check. */
const isAction = (v: unknown): v is ActionInfo => shaped<ActionInfo>(v, { id: isStr, label: isStr, hint: isStr, model: isBool, dictation: isBool });
/** Whether a value is one recorded run. Input: the value. Return: the check. */
const isLogEntry = (v: unknown): v is LogEntry => shaped<LogEntry>(v, { at: isNum, trigger: (t) => oneOf(TRIGGERS, t), matched: strings, app: isStr });

/** The `get_rules` reply as rules. Input: the raw reply. Return: the rules, or null when malformed. */
export const parseRules = (raw: unknown): Rule[] | null => (listOf(isRule)(raw) ? raw : null);
/** The `list_actions` reply as actions. Input: the raw reply. Return: the actions, or null when malformed. */
export const parseActions = (raw: unknown): ActionInfo[] | null => (listOf(isAction)(raw) ? raw : null);
/** The `rule_log` reply as runs. Input: the raw reply. Return: the runs, or null when malformed. */
export const parseLog = (raw: unknown): LogEntry[] | null => (listOf(isLogEntry)(raw) ? raw : null);

/** The stored rules. Return: resolves to them; rejects on a refusal or a malformed reply, so nobody saves over rules that could not be read. */
export const getRules = () => read("get_rules", parseRules);
/** Stores the rules. Input: the rules. Return: resolves when saved, rejects with Rust's reason. */
export const saveRules = (rules: Rule[]) => invoke("save_rules", { rules });
/** The actions a rule can run. Return: resolves to them; rejects on a refusal or a malformed reply. */
export const listActions = () => read("list_actions", parseActions);
/** Runs the rules on a sample text without typing anything. Input: the rules, trigger, text, program and whether the model may run. Return: what would happen. */
export const testRules = (rules: Rule[], trigger: Trigger, text: string, app: string, withModel = false) =>
  invoke<TestResult>("test_rules", { rules, trigger, text, app, withModel });
/** The recent runs. Return: resolves to them; rejects on a refusal or a malformed reply. */
export const getLog = () => read("rule_log", parseLog);
