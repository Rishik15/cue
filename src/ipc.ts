/**
 * File: ipc.ts
 * Purpose: The checks every IPC adapter validates Rust's replies and events with: a check takes `unknown` and says whether it has its shape, and a malformed read is shown to the user, never replaced by a writable default.
 * Contents: isRecord / isStr / isNum / isBool — value checks; strings / listOf — list checks; nullable — a check that also lets null through; shaped — an object whose every field passes its check; oneOf — a value from a label table;
 * read — invoke a command and check its reply, rejecting with the caller's message when the reply has the wrong shape; reason — the text to show for a rejection.
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
/** A check that also lets null through, for a reply where null means nothing happened. Input: the check. Return: the widened check. */
export const nullable = <T>(ok: (v: unknown) => v is T) => (v: unknown): v is T | null => v === null || ok(v);

/** One check per field of `T`, so a field cannot be left unchecked. */
export type Checks<T> = { [K in keyof T]-?: (v: unknown) => boolean };
/** Whether a value is an object whose every field passes its check. Input: the value and the checks. Return: the check. */
export const shaped = <T>(v: unknown, checks: Checks<T>): v is T =>
  isRecord(v) && Object.entries<(f: unknown) => boolean>(checks).every(([key, ok]) => ok(v[key]));

/** Whether a value is one of the values of a label table. Input: the table and the value. Return: the check. */
export const oneOf = <T extends string>(table: Record<string, T>, v: unknown): v is T => (Object.values(table) as unknown[]).includes(v);

/** Invokes a command and checks its reply. Input: the command, the check, the sentence the user sees when the reply has the wrong shape (what failed and the way out), and the arguments. Return: resolves to the reply; rejects with the command's refusal, or with an Error carrying `malformed`. */
export const read = <T>(command: string, ok: (raw: unknown) => raw is T, malformed: string, args?: InvokeArgs): Promise<T> =>
  invoke(command, args).then((raw) => (ok(raw) ? raw : Promise.reject(new Error(malformed))));

/** The text to show for a rejection: an Error's message, or a refusal from Rust as it came. Input: the rejection. Return: the text. */
export const reason = (e: unknown) => (e instanceof Error ? e.message : String(e));
