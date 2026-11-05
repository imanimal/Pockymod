import { describe, expect, it } from "vitest";
import { parse, safeHref, toText } from "./bbcode";

describe("bbcode", () => {
  it("keeps the safe subset", () => {
    const n = parse("[b]Bold[/b] and [i]italic[/i]\n[list][*]one[*]two[/list]");
    expect(n[0]).toMatchObject({ t: "b", c: [{ t: "text", v: "Bold" }] });
    expect(toText(n)).toContain("• one\n• two");
  });
  it("only keeps http(s) links", () => {
    expect(safeHref("javascript:alert(1)")).toBeNull();
    expect(safeHref("https://nexusmods.com")).toBe("https://nexusmods.com");
    const n = parse("[url=javascript:alert(1)]click[/url]");
    expect(JSON.stringify(n)).not.toContain("javascript");
    const ok = parse("[url=https://x.test/a]x[/url]");
    expect(ok[0]).toMatchObject({ t: "link", href: "https://x.test/a" });
  });
  it("strips raw HTML and unknown tags keep their text", () => {
    const n = parse("<script>alert(1)</script>[size=5]Big[/size] [weird]x[/weird]");
    const text = toText(n);
    expect(text).not.toContain("<script>");
    expect(text).toContain("Big");
    expect(text).toContain("[weird]x[/weird]");
  });
  it("survives broken nesting", () => {
    expect(() => parse("[b][i]x[/b][/i][list][*]a")).not.toThrow();
    expect(toText(parse("[b]unclosed"))).toBe("unclosed");
  });
});
