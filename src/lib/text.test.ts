import { describe, expect, it } from "vitest";
import { descriptionLinks, descriptionParts, descriptionText } from "./text";

describe("descriptionText", () => {
  it("drops BBCode and HTML but keeps lines", () => {
    const src = "[b][color=#f3e500][size=5]ARCHIVE-XL[/size][/color][/b]\n<br />\n<br />ArchiveXL is a <i>modding</i> tool &amp; more[list][*]one[*]two[/list]";
    expect(descriptionText(src)).toBe("ARCHIVE-XL\n\nArchiveXL is a modding tool & more\n• one\n• two");
  });
  it("removes images and keeps link text", () => {
    expect(descriptionText("[img]https://x/y.png[/img]See [url=https://a]the wiki[/url]")).toBe("See the wiki");
  });
  it("never returns markup", () => {
    expect(descriptionText("<script>alert(1)</script>ok")).toBe("alert(1)ok");
  });
});

describe("descriptionParts", () => {
  it("keeps BBCode, HTML and bare links clickable", () => {
    const src = "Needs [url=https://www.nexusmods.com/cyberpunk2077/mods/107][b]CET[/b][/url], see <a href=\"https://wiki.redmodding.org/\">the wiki</a> or https://github.com/x/y. [url]www.example.com/a?b=1&amp;c=2[/url]";
    expect(descriptionParts(src)).toEqual([
      "Needs ", { text: "CET", url: "https://www.nexusmods.com/cyberpunk2077/mods/107" }, ", see ", { text: "the wiki", url: "https://wiki.redmodding.org/" },
      " or ", { text: "https://github.com/x/y", url: "https://github.com/x/y" }, ". ", { text: "https://www.example.com/a?b=1&c=2", url: "https://www.example.com/a?b=1&c=2" },
    ]);
  });
  it("leaves out what is not a web page", () => {
    expect(descriptionParts("[url=javascript:alert(1)]x[/url] [url=/cyberpunk2077/mods/5]y[/url]")).toEqual(["x ", { text: "y", url: "https://www.nexusmods.com/cyberpunk2077/mods/5" }]);
  });
  it("lists each link once with its words", () => {
    const parts = descriptionParts("[url=https://a.com/]A[/url] [url=http://a.com]again[/url] https://b.org");
    expect(descriptionLinks(parts)).toEqual([{ url: "https://a.com/", labels: ["A", "again"] }, { url: "https://b.org/", labels: [] }]);
  });
});
