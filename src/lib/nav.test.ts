import { describe, expect, it } from "vitest";
import { settingsTarget } from "./nav";

describe("settingsTarget", () => {
  it("finds a named section", () => {
    expect(settingsTarget("Take them back in Settings, Nexus pages")).toEqual({ section: "Nexus pages" });
    expect(settingsTarget("Pockymod installed. Pick it in Settings, Appearance")).toEqual({ section: "Appearance" });
    expect(settingsTarget("Built-in ones are in Settings, Game and mod folders.")).toEqual({ section: "Game and mod folders" });
  });
  it("places a bare Settings by what the message is about", () => {
    expect(settingsTarget("No Nexus API key. Add one in Settings")).toEqual({ section: "Nexus account" });
    expect(settingsTarget("Set the Cyberpunk 2077 folder in Settings first")).toEqual({ section: "Game and mod folders" });
    expect(settingsTarget("Turn on the copy fallback in Settings")).toEqual({});
  });
  it("leaves mod names and other uses alone", () => {
    expect(settingsTarget("Native Settings UI queued")).toBeNull();
    expect(settingsTarget("Mod Settings installed")).toBeNull();
    expect(settingsTarget("Queued. It installs by itself")).toBeNull();
  });
});
