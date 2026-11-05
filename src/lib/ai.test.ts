import { describe, expect, it } from "vitest";
import { aiExcluded, aiKinds, withAiHidden } from "./ai";

const q = (tags: string[] = [], excludeTags: string[] = []) => ({ tags, excludeTags, text: "x" });

describe("aiKinds", () => {
  it("reads Nexus's three tags, most AI first", () => {
    expect(aiKinds([{ name: "Gameplay" }, { name: "AI Assisted" }, { name: "AI-Generated Content" }])).toEqual(["generated", "assisted"]);
    expect(aiKinds([{ name: "ai media" }])).toEqual(["media"]);
  });
  it("does not take the game's own AI for it", () => {
    expect(aiKinds([{ name: "NPC Behaviours (AI)" }, { name: "AI Companions" }])).toEqual([]);
    expect(aiKinds(undefined)).toEqual([]);
  });
});

describe("withAiHidden", () => {
  it("adds nothing when AI is shown", () => {
    const base = q();
    expect(withAiHidden(base, "show")).toBe(base);
  });
  it("leaves out generated, or all three", () => {
    expect(withAiHidden(q(), "generated").excludeTags).toEqual(["AI-Generated Content"]);
    expect(withAiHidden(q([], ["Sexualised"]), "all").excludeTags).toEqual(["Sexualised", "AI-Generated Content", "AI Media", "AI Assisted"]);
    expect(aiExcluded("all")).toHaveLength(3);
  });
  it("keeps a tag the user asked for by name, and does not repeat one", () => {
    expect(withAiHidden(q(["AI Assisted"]), "all").excludeTags).toEqual(["AI-Generated Content", "AI Media"]);
    const base = q([], ["AI Media"]);
    expect(withAiHidden(base, "all").excludeTags).toEqual(["AI Media", "AI-Generated Content", "AI Assisted"]);
  });
});
