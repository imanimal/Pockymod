import { describe, expect, it } from "vitest";
import { descriptionMedia } from "./api";

describe("descriptionMedia", () => {
  it("collects pictures and videos once each", () => {
    const d = "[img]https://i.imgur.com/a.png[/img] <img src=\"https://x/b.jpg\"> [youtube]dQw4w9WgXcQ[/youtube] https://youtu.be/dQw4w9WgXcQ [img]https://i.imgur.com/a.png[/img]";
    const m = descriptionMedia(d, "https://staticdelivery.nexusmods.com/p.png");
    expect(m.map((x) => x.kind)).toEqual(["image", "image", "image", "video"]);
    expect(m[3].url).toBe("https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ");
  });
});
