/**
 * File: ipc.ts
 * Purpose: The checks every IPC adapter parses Rust's replies and events with: an adapter takes `unknown` and returns its shape or null, and a malformed read is shown to the user, never replaced by a writable default.
 * Contents: isRecord / isStr / isNum / isBool — value checks; strings / listOf — list checks; shaped — an object whose every field passes its check; oneOf — a value from a label table;
 * unreadable — the message for a reply that has the wrong shape; read — invoke a command and parse its reply, rejecting with that message.
 */
import { invoke, type InvokeArgs } from "@tauri-apps/api/core";

/** Whether a value is a plain object (not null, not a list). Input: the value. Return: the check. */
export const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
/** Whether a value is a string. Input: the value. Return: the check. */
export const isStr = (v: unknown): v is string => typeof v === "string";
/** Whether a value is a finite number. Input: the value. Return: the check. */
export const isNum = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);
/** Whether a value is a boolean. Input: the value. Return: the check. */
export const isBool = (v: unknown): v is boolean => typeof v === "boolean";

/** A check for a list whose every element passes `ok`. Input: the element check. Return: the list check. */
export const listOf = <T>(ok: (e: unknown) => e is T) => (v: unknown): v is T[] => Array.isArray(v) && v.every(ok);
/** Whether a value is a list of strings. Input: the value. Return: the check. */
export const strings = listOf(isStr);

/** One check per field of `T`, so a field cannot be left unchecked. */
export type Checks<T> = { [K in keyof T]-?: (v: unknown) => boolean };
/** Whether a value is an object whose every field passes its check. Input: the value and the checks. Return: the check. */
export const shaped = <T>(v: unknown, checks: Checks<T>): v is T =>
  isRecord(v) && Object.entries<(f: unknown) => boolean>(checks).every(([key, ok]) => ok(v[key]));

/** Whether a value is one of the values of a label table. Input: the table and the value. Return: the check. */
export const oneOf = <T extends string>(table: Record<string, T>, v: unknown): v is T => (Object.values(table) as unknown[]).includes(v);

/** What the user is told when a reply has the wrong shape. Input: the command. Return: the message. */
export const unreadable = (command: string) => `Cue could not read the answer to ${command}.`;

/** Invokes a command and parses its reply. Input: the command, the parser and the arguments. Return: resolves to the parsed reply; rejects with the command's refusal, or with `unreadable` when the reply has the wrong shape. */
export const read = <T>(command: string, parse: (raw: unknown) => T | null, args?: InvokeArgs): Promise<T> =>
  invoke(command, args).then((raw) => parse(raw) ?? Promise.reject(new Error(unreadable(command))));
