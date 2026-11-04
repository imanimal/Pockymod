import { describe, expect, it } from "vitest";
import { descriptionText } from "./text";

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
