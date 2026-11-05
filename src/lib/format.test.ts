import { describe, expect, it } from "vitest";
import { ago, bytes, bytesParts, count, duration, errMsg } from "./format";

describe("format", () => {
  it("bytes", () => {
    expect(bytes(0)).toBe("0 B");
    expect(bytes(1536)).toBe("1.5 KB");
    expect(bytes(22 * 1024 * 1024)).toBe("22 MB");
    expect(bytes(null)).toBe("–");
    expect(bytesParts(3.2 * 1024 ** 3)).toEqual(["3.2", "GB"]);
  });
  it("counts", () => {
    expect(count(999)).toBe("999");
    expect(count(1234)).toBe("1.2K");
    expect(count(98_000)).toBe("98K");
    expect(count(9_800_000)).toBe("9.8M");
  });
  it("durations and ages", () => {
    expect(duration(125)).toBe("2:05");
    expect(duration(3725)).toBe("1:02:05");
    expect(duration(null)).toBe("–");
    const now = 1_800_000_000;
    expect(ago(now - 100, now)).toBe("today");
    expect(ago(now - 3 * 86400, now)).toBe("3 days ago");
    expect(ago(now - 70 * 86400, now)).toBe("2 months ago");
  });
  it("error messages", () => {
    expect(errMsg({ kind: "noKey", message: "Add your key" })).toBe("Add your key");
    expect(errMsg("plain")).toBe("plain");
  });
});
