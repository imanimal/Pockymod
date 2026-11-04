import { describe, expect, it } from "vitest";
import { extractCommands } from "./commands";

describe("extractCommands", () => {
  it("finds Game.* calls, once each, in order", () => {
    const d = "Spawn with:\nGame.AddToInventory(\"Items.MyJacket\", 1)\nand Game.AddToInventory(\"Items.MyJacket\", 1);\nGame.GiveDevPoints(\"Primary\", 5)";
    expect(extractCommands(d).map((c) => c.command)).toEqual(['Game.AddToInventory("Items.MyJacket", 1)', 'Game.GiveDevPoints("Primary", 5)']);
  });
  it("fixes curly quotes and keeps nested brackets", () => {
    expect(extractCommands("Game.AddToInventory(“Items.Hat”, 1)")[0].command).toBe('Game.AddToInventory("Items.Hat", 1)');
    expect(extractCommands("Game.TeleportPlayerToPosition(ToVector4(1, 2, 3))")[0].command).toBe("Game.TeleportPlayerToPosition(ToVector4(1, 2, 3))");
  });
  it("turns bare item IDs into AddToInventory lines, marked as generated", () => {
    const c = extractCommands("Item codes: Items.Boots_Red, Items.Boots_Blue\nGame.AddToInventory(\"Items.Boots_Red\", 1)");
    expect(c).toEqual([
      { command: 'Game.AddToInventory("Items.Boots_Red", 1)', generated: false },
      { command: 'Game.AddToInventory("Items.Boots_Blue", 1)', generated: true },
    ]);
  });
  it("finds nothing in plain text", () => {
    expect(extractCommands("A nice jacket. Requires ArchiveXL.")).toEqual([]);
  });
});
