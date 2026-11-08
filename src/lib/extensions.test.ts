import { describe, expect, it } from "vitest";
import { approvalItems, compareVersions, validateExtension, validateIndex, validateTheme, type ExtensionManifest } from "./addon-rules.mjs";
import { collect, planGroups, readInstalled, ruleMatches, shelfQuery } from "./extensions";
import { makeBridge } from "./panel-bridge";
import { submissionUrl } from "./market";
import type { InstallRow } from "./api";

const base = (): ExtensionManifest => ({
  format: "pocket-mods.extension", version: 1, id: "test-ext", name: "Test", release: "1.0.0",
  contributes: { shelves: [{ id: "poses", title: "Poses", query: { tags: ["Poses"], sort: "endorsements" } }] },
});

describe("extension manifests", () => {
  it("accepts a small valid one", () => {
    expect(validateExtension(base())).toEqual({ ok: true, errors: [], warnings: [] });
  });
  it("names the field that is wrong", () => {
    const m = { ...base(), version: 2, permissions: ["everything"], contributes: { shelves: [{ id: "Bad Id", title: "x", query: { sort: "best", colour: 1 } }] } };
    const r = validateExtension(m);
    expect(r.ok).toBe(false);
    expect(r.errors.join("\n")).toMatch(/^version:/m);
    expect(r.errors.join("\n")).toMatch(/permissions\[0\]: "everything" is not a permission/);
    expect(r.errors.join("\n")).toMatch(/contributes\.shelves\[0\]\.id/);
    expect(r.errors.join("\n")).toMatch(/query\.sort: must be one of/);
    expect(r.errors.join("\n")).toMatch(/query\.colour: is not a field/);
  });
  it("keeps tools inside the game folder and arguments plain", () => {
    for (const path of ["../x.exe", "C:\\x.exe", "/x.exe", "tools/../../x.exe", "tools/x.bat"]) {
      const m = { ...base(), contributes: { tools: [{ id: "t", name: "T", path }] } };
      expect(validateExtension(m).ok, path).toBe(false);
    }
    expect(validateExtension({ ...base(), contributes: { tools: [{ id: "t", name: "T", path: "tools/redmod/bin/redMod.exe", args: ["-x"] }] } }).ok).toBe(true);
    expect(validateExtension({ ...base(), contributes: { tools: [{ id: "t", name: "T", pick: { file: "WolvenKit.exe" }, args: ['a"b'] }] } }).ok).toBe(false);
    expect(validateExtension({ ...base(), contributes: { tools: [{ id: "t", name: "T" }] } }).ok).toBe(false);
    expect(validateExtension({ ...base(), contributes: { launchOptions: [{ id: "l", name: "L", args: ["-a b"] }] } }).ok).toBe(false);
    expect(validateExtension({ ...base(), contributes: { links: [{ id: "l", title: "L", url: "http://example.com" }] } }).ok).toBe(false);
  });
  it("lists what needs approving", () => {
    const m: ExtensionManifest = { ...base(), permissions: ["toast"], contributes: {
      tools: [{ id: "wk", name: "WK", pick: { file: "WolvenKit.exe" } }], launchOptions: [{ id: "s", name: "S", args: ["-skipStartScreen"] }],
      links: [{ id: "w", title: "W", url: "https://wiki.redmodding.org/" }], panels: [{ id: "p", title: "P", html: "<p>hi</p>" }] } };
    expect(approvalItems(m)).toEqual(['["permission","toast"]', '["tool","wk","","WolvenKit.exe",[],""]', '["launch","s",["-skipStartScreen"]]', `["link","${m.contributes.links?.[0].id ?? ""}","https://wiki.redmodding.org/"]`]);
    // a path that looks like a pick, or arguments joined differently, are different items
    const a = approvalItems({ ...m, contributes: { tools: [{ id: "wk", name: "W", path: "pick:WolvenKit.exe" }] } } as ExtensionManifest);
    const b = approvalItems({ ...m, contributes: { tools: [{ id: "wk", name: "W", pick: { file: "WolvenKit.exe" } }] } } as ExtensionManifest);
    expect(a.filter((x) => x.startsWith('["tool"'))).not.toEqual(b.filter((x) => x.startsWith('["tool"')));
  });
});

describe("installed state", () => {
  const text = JSON.stringify({ ...base(), permissions: ["toast"], contributes: { ...base().contributes, panels: [{ id: "p", title: "P", html: "x" }] } });
  it("does nothing until approved", () => {
    const x = readInstalled(text, {});
    expect(x.status).toBe("review");
    expect(collect([x]).shelves).toHaveLength(0);
  });
  it("applies when on and approved for this release", () => {
    const state = { enabled: true, approvedRelease: "1.0.0", approved: ["permission:toast"], toolPaths: {}, fromMarket: false, installedAt: 0 };
    const x = readInstalled(text, { "test-ext": state });
    expect(x.status).toBe("on");
    expect(collect([x]).shelves[0]).toMatchObject({ id: "poses", extId: "test-ext", extName: "Test" });
    expect(collect([x]).panels[0].permissions).toEqual(["toast"]);
    expect(readInstalled(text, { "test-ext": { ...state, enabled: false } }).status).toBe("off");
    expect(readInstalled(text, { "test-ext": { ...state, approvedRelease: "0.9.0" } }).status).toBe("review");
  });
  it("marks a broken file", () => {
    expect(readInstalled("{ nope", {}).status).toBe("broken");
  });
});

const row = (p: Partial<InstallRow>): InstallRow => ({
  id: 1, modId: 10, fileId: 1, name: "Mod", version: "1", modType: "archive", frameworks: [], staging: "", installedAt: 0, updatedAt: null, deployedFiles: 1,
  enabled: true, loadOrder: 0, source: "pocket", category: "Gameplay", group: "Gameplay", customGroup: false, ...p,
});

describe("group rules", () => {
  it("match by name part, category, author, type, framework or id", () => {
    const r = (match: object) => ({ group: "G", match });
    expect(ruleMatches(r({ name: ["photo mode"] }), row({ name: "Better Photo Mode Poses" }))).toBe(true);
    expect(ruleMatches(r({ category: ["animations"] }), row({ category: "Animations" }))).toBe(true);
    expect(ruleMatches(r({ author: ["psiberx"] }), row({ author: "psiberx" }))).toBe(true);
    expect(ruleMatches(r({ author: ["psiberx"] }), row({ author: null }))).toBe(false);
    expect(ruleMatches(r({ frameworks: ["cet"] }), row({ frameworks: ["cet"] }))).toBe(true);
    expect(ruleMatches(r({ mods: [10] }), row({}))).toBe(true);
    expect(ruleMatches(r({ name: ["photo"] }), row({ name: "Gameplay fix" }))).toBe(false);
  });
  it("leave your own groups alone and take the first match", () => {
    const rules = [{ group: "Photo", match: { name: ["pose"] } }, { group: "Other", match: { name: ["pose"] } }];
    const plan = planGroups([row({ id: 1, name: "Pose pack" }), row({ id: 2, name: "Pose pack 2", customGroup: true, group: "Mine" }), row({ id: 3, name: "Pose pack 3", group: "Photo" })], rules);
    expect(plan.map((p) => [p.row.id, p.group])).toEqual([[1, "Photo"]]);
  });
});

describe("shelves", () => {
  it("turn into Browse query fields", () => {
    const now = Date.UTC(2026, 10, 4);
    expect(shelfQuery({ tags: ["Poses"], createdWithinDays: 7, sort: "random" }, now)).toEqual({ tags: ["Poses"], createdAfter: now / 1000 - 7 * 86400, sort: "random", seed: Math.floor(now / 86_400_000) });
  });
});

describe("panel bridge", () => {
  const info = { extId: "x", extName: "X", release: "1.0.0", panelId: "p", title: "P", permissions: ["toast" as const] };
  const ui = { toast: () => {}, confirmQueue: async () => false };
  it("refuses what was not declared", async () => {
    const call = makeBridge(info, ui);
    await expect(call("library.list", null)).rejects.toThrow(/did not ask for the library.read permission/);
    await expect(call("queue.add", { modId: 1 })).rejects.toThrow(/queue.add permission/);
    await expect(call("app.secret", null)).rejects.toThrow(/no method/);
  });
  it("checks arguments and limits notes", async () => {
    const shown: string[] = [];
    const call = makeBridge(info, { ...ui, toast: (t) => shown.push(t) });
    await expect(call("toast", { text: 5 })).rejects.toThrow(/text must be text/);
    await call("toast", { text: "hi" });
    expect(shown).toEqual(["X: hi"]);
    await call("toast", { text: "2" });
    // three notes every five seconds, and the refused call counts too
    await expect(call("toast", { text: "3" })).rejects.toThrow(/Too many/);
  });
});

describe("marketplace", () => {
  it("compares versions", () => {
    expect(compareVersions("1.10.0", "1.9.3")).toBe(1);
    expect(compareVersions("1.0.0", "1.0.0")).toBe(0);
    expect(compareVersions("1.0.0-beta", "1.0.0")).toBe(-1);
    expect(compareVersions("2.0.0", "nope")).toBe(1);
  });
  it("checks the index", () => {
    const entry = { id: "neon", kind: "theme", name: "Neon", author: "A", version: "1.0.0", description: "D", path: "themes/neon.json", updated: "2026-11-04" };
    expect(validateIndex({ format: "pocket-mods.marketplace", version: 1, themes: [entry], extensions: [] }).ok).toBe(true);
    const r = validateIndex({ format: "pocket-mods.marketplace", version: 1, themes: [entry, { ...entry, path: "../x.json", version: "1" }], extensions: [] });
    expect(r.errors.join("\n")).toMatch(/themes\[1\]\.id: neon is listed twice/);
    expect(r.errors.join("\n")).toMatch(/themes\[1\]\.version/);
    expect(r.errors.join("\n")).toMatch(/themes\[1\]\.path/);
  });
  it("checks themes", () => {
    const t = { format: "pocket-mods.theme", version: 1, id: "t", name: "T", scheme: "dark", tokens: { "--bg": "#000" } };
    expect(validateTheme(t).ok).toBe(true);
    expect(validateTheme({ ...t, tokens: { bg: "#000" } }).ok).toBe(false);
    expect(validateTheme({ ...t, tokens: { "--bg": "url(https://x/y.png)" } }).ok).toBe(false);
    expect(validateTheme({ ...t, css: "@import 'https://x';" }).ok).toBe(false);
  });
  it("fills in the submission form, and leaves out a package too long for the address", () => {
    const small = submissionUrl("theme", "Neon", '{"id":"neon"}');
    expect(small.withPackage).toBe(true);
    const u = new URL(small.url);
    expect(u.origin + u.pathname).toBe("https://github.com/imanimal/Pockymod/issues/new");
    expect(u.searchParams.get("template")).toBe("marketplace-submission.yml");
    expect(u.searchParams.get("kind")).toBe("Theme");
    expect(u.searchParams.get("package")).toBe('{"id":"neon"}');
    const big = submissionUrl("extension", "Big", JSON.stringify({ html: "x".repeat(20000) }));
    expect(big.withPackage).toBe(false);
    expect(new URL(big.url).searchParams.get("package")).toBeNull();
  });
});
