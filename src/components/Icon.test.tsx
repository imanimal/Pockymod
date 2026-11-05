import { describe, expect, it } from "vitest";
import { ICON_NAMES, bitmap } from "./Icon";

describe("pixel icons", () => {
  it("are 9×9 bitmaps of 0/1 pixels", () => {
    for (const n of ICON_NAMES) {
      const rows = bitmap(n);
      expect(rows.length, n).toBe(9);
      for (const r of rows) expect(r, n).toMatch(/^[.1]{9}$/);
      expect(rows.join("").includes("1"), n).toBe(true);
    }
  });
});
