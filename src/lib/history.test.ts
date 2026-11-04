import { describe, expect, it, vi } from "vitest";
vi.mock("../ui/toast", () => ({ toast: () => 1, retoast: () => true }));
import { record, redo, undo, entries } from "./history";

describe("undo and redo", () => {
  it("runs the inverse, redoes it, and drops the redo chain after a new change", async () => {
    let on = true;
    const off = () => { on = false; return Promise.resolve(); };
    const back = () => { on = true; return Promise.resolve(); };
    await off();
    const e = record({ label: "Turned off A", undo: back, redo: off }, true);
    expect(await undo()).toBe(true);
    expect(on).toBe(true);
    expect(await redo()).toBe(true);
    expect(on).toBe(false);
    expect(await undo(e)).toBe(true);
    // a new change: the undone entry can no longer be redone
    record({ label: "Moved B", undo: () => Promise.resolve(), redo: () => Promise.resolve() }, true);
    expect(await redo(e)).toBe(false);
    expect(on).toBe(true);
    expect(entries().find((x) => x.id === e.id)?.state).toBe("gone");
  });

  it("undoing an older change first undoes the newer ones", async () => {
    const log: string[] = [];
    const a = record({ label: "a", undo: async () => { log.push("undo a"); }, redo: async () => { log.push("redo a"); } }, true);
    record({ label: "b", undo: async () => { log.push("undo b"); }, redo: async () => { log.push("redo b"); } }, true);
    expect(await undo(a)).toBe(true);
    expect(log).toEqual(["undo b", "undo a"]);
  });
});
