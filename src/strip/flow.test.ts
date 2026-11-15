/**
 * File: strip/flow.test.ts
 * Purpose: Protect the Transform Text state machine: the strip's keys (1 to 3, Down, M, typing, Esc), the list's (loop round, Enter runs), the result's (Enter pastes only a change, R retries only a model row, Esc returns to the view the result came from, the strip or the list with its search kept, and cancels a working run), exact rows showing their result with no command (or asking for it when it was left out), a late or stale model reply never changing the screen, and a run that did not finish saying so.
 * Contents: cases for start, key, pick, hover and finish.
 */
import { describe, expect, it } from "vitest";
import { finish, hover, key, pick, query, start, type Flow, type Row } from "./flow";

const TEXT = "hello world";
const ROWS: Row[] = [
  { id: "action:upper_case", name: "Uppercase", hint: "", group: "convert", model: false, result: "HELLO WORLD" },
  { id: "action:correct", name: "Correct Text", hint: "", group: "ai", model: true, result: null },
  { id: "action:title_case", name: "Title Case", hint: "", group: "convert", model: false, result: "Hello World" },
];
const fresh = () => start(ROWS, TEXT, [0, 1]);

/** Presses keys in order. Input: the flow and the keys. Return: the final flow and the last effect. */
function press(f: Flow, ...keys: string[]) {
  let now = f;
  let effect = { kind: "none" } as ReturnType<typeof key>[1];
  for (const k of keys) [now, effect] = key(now, k);
  return { f: now, effect };
}

describe("strip", () => {
  it("runs a chip by its number: an exact row shows its result at once", () => {
    const { f, effect } = press(fresh(), "1");
    expect(effect.kind).toBe("none");
    expect(f.view).toBe("result");
    expect(f.outcome).toEqual({ kind: "done", text: "HELLO WORLD" });
  });

  it("asks Rust to run a model chip and ignores a number with no chip", () => {
    expect(press(fresh(), "2").effect).toEqual({ kind: "run", row: 1, seq: 1 });
    expect(press(fresh(), "3").f.view).toBe("strip");
  });

  it("opens the list on Down, M or More's 4 with an empty search, and on another letter searching it", () => {
    expect(press(fresh(), "ArrowDown").f).toMatchObject({ view: "list", query: "" });
    expect(press(fresh(), "m").f).toMatchObject({ view: "list", query: "" });
    expect(press(fresh(), "4").f).toMatchObject({ view: "list", query: "" });
    expect(press(fresh(), "t").f).toMatchObject({ view: "list", query: "t" });
    expect(press(fresh(), "Escape").effect).toEqual({ kind: "close" });
  });
});

describe("list", () => {
  it("moves round the list both ways and runs the highlighted row on Enter", () => {
    expect(press(fresh(), "ArrowDown", "ArrowUp").f.selected).toBe(2);
    const { f } = press(fresh(), "ArrowDown", "ArrowDown", "ArrowDown", "ArrowDown", "Enter");
    expect(f.row).toBe(0);
    expect(f.view).toBe("result");
  });

  it("follows the pointer and steps back to the strip on Esc", () => {
    expect(hover(press(fresh(), "ArrowDown").f, 2).selected).toBe(2);
    expect(hover(fresh(), 2).selected).toBe(0);
    expect(press(fresh(), "ArrowDown", "Escape").f.view).toBe("strip");
  });
});

describe("result", () => {
  it("pastes only a change; Enter on no change goes back to where the result came from", () => {
    expect(press(fresh(), "1", "Enter").effect).toEqual({ kind: "paste", row: 0 });
    const same = start([{ ...ROWS[0], result: TEXT } as Row], TEXT, [0]);
    const r = press(same, "1");
    expect(r.f.outcome.kind).toBe("unchanged");
    expect(press(r.f, "Enter").f.view).toBe("strip");
  });

  it("cancels a working model run on Esc, and its late reply changes nothing", () => {
    const working = press(fresh(), "2").f;
    const back = key(working, "Escape");
    expect(back[1]).toEqual({ kind: "cancel" });
    expect(finish(back[0], 1, "Hello, world.")).toBe(back[0]);
  });

  it("settles the newest run only, and R runs a model row again once it has answered", () => {
    const working = press(fresh(), "2").f;
    expect(finish(working, 0, "old")).toBe(working);
    const done = finish(working, 1, "Hello, world.");
    expect(done.outcome).toEqual({ kind: "done", text: "Hello, world." });
    expect(key(working, "r")[1]).toEqual({ kind: "none" });
    expect(key(done, "r")[1]).toEqual({ kind: "run", row: 1, seq: 2 });
    expect(key(press(fresh(), "1").f, "r")[1]).toEqual({ kind: "none" });
  });

  it("treats an empty or identical model answer as no change, and a run that did not finish as failed", () => {
    const working = press(fresh(), "2").f;
    expect(finish(working, 1, "  ").outcome.kind).toBe("unchanged");
    expect(finish(working, 1, TEXT).outcome.kind).toBe("unchanged");
    const failed = finish(working, 1, null);
    expect(failed.outcome.kind).toBe("failed");
    expect(key(failed, "Enter")[0].view).toBe("strip");
    expect(key(failed, "r")[1]).toEqual({ kind: "run", row: 1, seq: 2 });
  });

  it("asks Rust for an exact row's result when it came without one (a long text)", () => {
    const long = start([{ ...ROWS[0], result: null } as Row], TEXT, [0]);
    const [f, effect] = key(long, "1");
    expect(effect).toEqual({ kind: "run", row: 0, seq: 1 });
    expect(finish(f, 1, "HELLO WORLD").outcome).toEqual({ kind: "done", text: "HELLO WORLD" });
  });

  it("runs a clicked row and ignores a row that does not exist", () => {
    expect(pick(fresh(), 2)[0].outcome).toEqual({ kind: "done", text: "Hello World" });
    expect(pick(fresh(), 9)[0].view).toBe("strip");
  });
});

describe("origin", () => {
  it("returns to the strip on Esc from a chip run by its key, a model chip, or a click (Enter on a focused chip is its click)", () => {
    expect(press(fresh(), "1", "Escape").f.view).toBe("strip");
    expect(key(pick(fresh(), 0)[0], "Escape")[0].view).toBe("strip");
    expect(press(fresh(), "2", "Escape").f.view).toBe("strip");
  });

  it("returns to the list with its search and highlight on Esc from a list row, by key or click", () => {
    const list = press(query(fresh(), "case"), "ArrowDown").f;
    const back = press(list, "Enter", "Escape").f;
    expect(back).toMatchObject({ view: "list", query: "case", selected: list.selected });
    expect(back.hits).toEqual(list.hits);
    expect(key(pick(list, 2)[0], "Escape")[0]).toMatchObject({ view: "list", query: "case" });
    expect(press(back, "Escape").f.view).toBe("strip");
  });

  it("keeps the origin across a retry", () => {
    const fromList = finish(press(fresh(), "ArrowDown", "Enter").f, 1, "Hello, world.");
    expect(fromList.from).toBe("list");
    const again = finish(key(fromList, "r")[0], 2, "Hi.");
    expect(again.view).toBe("result");
    expect(key(again, "Escape")[0].view).toBe("list");
  });
});

describe("compare", () => {
  it("toggles the original with O on a change only, keeps pasting the result, and shows the result again on a new run or on leaving", () => {
    const done = press(fresh(), "1").f;
    expect(done.compare).toBe(false);
    const original = press(done, "o");
    expect(original.f.compare).toBe(true);
    expect(press(original.f, "O").f.compare).toBe(false);
    expect(key(original.f, "Enter")[1]).toEqual({ kind: "paste", row: 0 });
    expect(press(original.f, "r").f.compare).toBe(true);
    const model = key(finish(press(fresh(), "2").f, 1, "Hello, world."), "o")[0];
    expect(model.compare).toBe(true);
    expect(press(model, "r").f.compare).toBe(false);
    expect(key(original.f, "Escape")[0].compare).toBe(false);
    const same = press(start([{ ...ROWS[0], result: TEXT } as Row], TEXT, [0]), "1", "o");
    expect(same.f.compare).toBe(false);
  });
});
