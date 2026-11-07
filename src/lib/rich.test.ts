import { describe, expect, it } from "vitest";
import { authorColor, parseRich, richMedia, type RichEl, type RichNode } from "./rich";

const el = (n: RichNode) => n as RichEl;

describe("parseRich", () => {
  it("reads a Nexus table with its cells, and drops the breaks between them", () => {
    const src = "[b]Keep 'Em Comin'[/b]\n<br />[center][table][tr][th][size=2][font=Verdana]Point[/font][/size][/th]\n<br />[th]Now[/th]\n<br />[/tr]\n<br />[tr][td]Stamina per kill[/td]\n<br />[td]+40 / +60[/td]\n<br />[/tr]\n<br />[/table][/center]\n<br />After";
    const t = parseRich(src);
    expect(el(t[0]).k).toBe("b");
    const center = el(t[1]);
    expect(center).toMatchObject({ k: "div", v: "center" });
    const table = el(center.kids[0]);
    expect(table.k).toBe("table");
    expect(table.kids.map((r) => el(r).k)).toEqual(["tr", "tr"]);
    expect(el(table.kids[0]).kids).toEqual([{ k: "th", tag: "[th", kids: ["Point"] }, { k: "th", tag: "[th", kids: ["Now"] }]);
    expect(el(el(table.kids[1]).kids[1]).kids).toEqual(["+40 / +60"]);
    expect(t[2]).toBe("After");
  });

  it("keeps pictures where they are written, with their width", () => {
    const t = parseRich("Before\n<br />[center][img width=665]https://staticdelivery.nexusmods.com/a.png[/img][/center]\n<br /><img src=\"https://x.org/b.jpg\" width=\"200\">");
    const media = richMedia(t);
    expect(media.map((m) => [m.k, m.v, m.w])).toEqual([["img", "https://staticdelivery.nexusmods.com/a.png", 665], ["img", "https://x.org/b.jpg", 200]]);
    expect(t[0]).toBe("Before");
  });

  it("reads YouTube videos written as a tag, an embed or an id", () => {
    const t = parseRich("[youtube]dQw4w9WgXcQ[/youtube] <iframe src=\"https://www.youtube.com/embed/abcdefghijk\"></iframe>");
    expect(richMedia(t).map((m) => [m.k, m.v])).toEqual([["video", "dQw4w9WgXcQ"], ["video", "abcdefghijk"]]);
  });

  it("makes lists, links and quotes", () => {
    const t = parseRich("[list][*]one [url=https://a.com][b]A[/b][/url]\n[*]two[/list][quote]said[/quote]");
    const list = el(t[0]);
    expect(list.k).toBe("ul");
    expect(list.kids.map((x) => el(x).k)).toEqual(["li", "li"]);
    expect(el(list.kids[0]).kids[1]).toMatchObject({ k: "link", v: "https://a.com/", kids: [{ k: "b", kids: ["A"] }] });
    expect(el(list.kids[1]).kids).toEqual(["two"]);
    expect(el(t[1])).toMatchObject({ k: "quote", kids: ["said"] });
  });

  it("leaves words in brackets alone and never keeps markup", () => {
    expect(parseRich("[REQUIRED] needs CET")).toEqual(["[REQUIRED] needs CET"]);
    expect(parseRich("<script>alert(1)</script>ok &amp; done")).toEqual(["alert(1)ok & done"]);
    expect(parseRich("[url=javascript:alert(1)]x[/url]")).toEqual(["x"]);
  });

  it("keeps the spaces around inline tags", () => {
    expect(parseRich("a [b] bold [/b] b")).toEqual(["a ", { k: "b", tag: "[b", kids: [" bold "] }, " b"]);
  });
});

describe("authorColor", () => {
  it("keeps colours and drops greys, white and black", () => {
    expect(authorColor("#00ffff")).toBe("#00ffff");
    expect(authorColor("#fff")).toBeNull();
    expect(authorColor("#D4D4D8")).toBeNull();
    expect(authorColor("orange")).toBe("orange");
    expect(authorColor("white")).toBeNull();
    expect(authorColor("red;background:url(x)")).toBeNull();
  });
});
