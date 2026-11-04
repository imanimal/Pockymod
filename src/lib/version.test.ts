import { describe, expect, it } from "vitest";
import { bump } from "./version";

describe("bump", () => {
  it("raises the last number", () => {
    expect(bump("1.28.6")).toBe("1.28.7");
    expect(bump("v2")).toBe("v3");
    expect(bump("1.0.9")).toBe("1.0.10");
    expect(bump(" 2.1.0-beta ")).toBe("2.1.1-beta");
  });
  it("leaves versions without a number alone", () => {
    expect(bump("final")).toBeNull();
    expect(bump("")).toBeNull();
  });
});
