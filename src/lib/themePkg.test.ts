import { describe, expect, it } from "vitest";
import neonHarbor from "../../docs/examples/neon-harbor.json?raw";
import paperTerminal from "../../docs/examples/paper-terminal.json?raw";
import { checkId, checkValue, compileTheme, packageSchemes, parseThemePackage, scopeCss, scopeSelector } from "./themePkg";

const pkg = (extra: Record<string, unknown> = {}) => JSON.stringify({ format: "pocket-mods.theme", version: 1, id: "my-theme", name: "Mine", scheme: "dark", ...extra });

describe("theme packages", () => {
  it("reads a small package", () => {
    const p = parseThemePackage(pkg({ tokens: { "--bg": "#101014", "--font-body": '"Space Grotesk", sans-serif' } }));
    expect(p.id).toBe("my-theme");
    expect(p.tokens?.["--bg"]).toBe("#101014");
    expect(packageSchemes(p)).toEqual(["dark"]);
  });

  it("knows a package with both schemes", () => {
    const p = parseThemePackage(pkg({ scheme: "light", dark: { "--bg": "#000" }, light: { "--bg": "#fff" } }));
    expect(packageSchemes(p)).toEqual(["light", "dark"]);
  });

  it("refuses what is not a theme", () => {
    expect(() => parseThemePackage("nope")).toThrow(/not valid JSON/);
    expect(() => parseThemePackage(JSON.stringify({ format: "x" }))).toThrow(/not a Pockymod theme/);
    expect(() => parseThemePackage(pkg({ version: 2 }))).toThrow(/version 1/);
    expect(() => parseThemePackage(pkg({ scheme: "dim" }))).toThrow(/scheme/);
    expect(() => parseThemePackage(pkg({ base: "other" }))).toThrow(/base/);
    expect(() => parseThemePackage(pkg({ name: "" }))).toThrow(/name/);
  });

  it("checks ids like the add-on store does", () => {
    expect(checkId("neon.v2-dark")).toBe("neon.v2-dark");
    for (const bad of ["", "My-Theme", "a b", "../x", ".x", "a..b", "x".repeat(65), "a/b", "standard", "pocket", "redline"]) {
      expect(() => checkId(bad)).toThrow();
    }
  });

  it("only takes custom properties as token names", () => {
    expect(() => parseThemePackage(pkg({ tokens: { color: "red" } }))).toThrow(/custom property/);
    expect(() => parseThemePackage(pkg({ tokens: { "--m": "0" } }))).toThrow(/set by the app/);
  });

  it("refuses token values that could reach outside the theme", () => {
    expect(checkValue("--x", "url(data:image/png;base64,AAAA)")).toContain("data:image");
    expect(checkValue("--x", "color-mix(in srgb, var(--accent) 30%, transparent)")).toContain("color-mix");
    for (const bad of [
      "url(https://example.com/x.png)", "url('http://x')", "url(file:///c:/x)", "red; color: blue", "red } body { display: none",
      "expression(alert(1))", 'image-set("https://x/a.png" 1x)', "red !important", "u\\72l(x)", "a /* b */", "rgb(1, 2", "red)", "javascript:x",
    ]) expect(() => checkValue("--x", bad), bad).toThrow();
  });

  it("compiles tokens to scoped rules", () => {
    const css = compileTheme(parseThemePackage(pkg({ tokens: { "--bg": "#111" }, light: { "--bg": "#eee" } })));
    expect(css).toContain('html:root[data-theme="my-theme"][data-scheme] {\n  --bg: #111;');
    expect(css).toContain('html:root[data-theme="my-theme"][data-scheme="light"]');
  });
});

describe("theme css", () => {
  const id = "my-theme";
  it("scopes every selector", () => {
    expect(scopeSelector(".btn", id)).toBe('html[data-theme="my-theme"] .btn');
    expect(scopeSelector(":root", id)).toBe('html[data-theme="my-theme"]');
    expect(scopeSelector(':root[data-scheme="dark"] .panel', id)).toBe('html[data-theme="my-theme"][data-scheme="dark"] .panel');
    expect(scopeSelector("html.night .x", id)).toBe('html[data-theme="my-theme"].night .x');
    expect(scopeSelector("body", id)).toBe('html[data-theme="my-theme"] body');
    expect(scopeSelector("htmlx", id)).toBe('html[data-theme="my-theme"] htmlx');
    const out = scopeCss(".btn, .chip:is(.on, .x) { color: red }", id);
    expect(out).toBe('html[data-theme="my-theme"] .btn, html[data-theme="my-theme"] .chip:is(.on, .x) { color: red }');
  });

  it("keeps media blocks and keyframes, drops imports and font faces", () => {
    const out = scopeCss(`@import url("https://x/y.css"); @charset "utf-8";
      @media (max-width: 900px) { .row { padding: 0 } }
      @keyframes glow { from { opacity: 0 } }
      @font-face { font-family: X; src: url(https://x/f.woff2) }
      .panel { animation: glow 1s }`, id);
    expect(out).not.toMatch(/@import|@charset|@font-face|https:/);
    expect(out).toContain('@media (max-width: 900px) {\nhtml[data-theme="my-theme"] .row { padding: 0 }\n}');
    expect(out).toContain("@keyframes glow");
    expect(out).toContain('html[data-theme="my-theme"] .panel');
  });

  it("turns external urls into none and keeps data images", () => {
    const out = scopeCss(".a { background: url(https://x/a.png) center, url('data:image/svg+xml;utf8,<svg/>') }", id);
    expect(out).toContain("background: none center, url('data:image/svg+xml;utf8,<svg/>')");
    expect(scopeCss('.a::after { content: "url(https://x)" }', id)).toContain('"url(https://x)"');
  });

  it("refuses css that breaks out of its block", () => {
    for (const bad of [
      ".a { color: red } } body { display: none }", ".a { color: red", "} .a {}", ".a { b: c } /* open",
      ".a { background: u\\72l(https://x) }", ".a { behavior: url(x.htc) }", ".a { b: expression(1) }", '.a { content: "open }',
      ".a { background: image-set(\"https://x/a.png\" 1x) }",
    ]) expect(() => scopeCss(bad, id), bad).toThrow();
  });

  it("drops @scope and other rules that could reach outside", () => {
    expect(scopeCss("@scope (body) { .a { color: red } }", id)).toBe("");
    expect(scopeCss("@media print { @font-face { font-family: x } .a { b: c } }", id)).toContain('html[data-theme="my-theme"] .a');
  });
});

describe("example themes in docs/examples", () => {
  for (const [f, text] of [["neon-harbor.json", neonHarbor], ["paper-terminal.json", paperTerminal]]) {
    it(`${f} checks out`, () => {
      const p = parseThemePackage(text);
      expect(compileTheme(p)).toContain(`html:root[data-theme="${p.id}"]`);
    });
  }
});

describe("theme css that tries to get out", async () => {
  const { parseThemePackage } = await import("./themePkg");
  const { validateTheme } = await import("./addon-rules.mjs");
  const base = { format: "pocket-mods.theme", version: 1, id: "evil", name: "Evil", scheme: "dark", tokens: { "--bg": "#000" } };
  const attacks = [
    'a { content: "x\r} body{background-image:image-set(\'https://evil/p.png\' 1x)} q{" }',
    'a { content: "x\f} body{background:red} q{" }',
    "a { background: url(data:image/png,x\"y)z\"w); content:\"A} body{color:red}\" }",
    "a { background: \\75 rl(https://evil/p.png) }",
    "a { background-image: image-set(\"https://evil/p.png\" 1x) }",
    "@import 'https://evil/x.css';",
    "@font-face { font-family: x; src: local(Arial) }",
  ];
  for (const css of attacks) {
    it(`refuses ${JSON.stringify(css).slice(0, 50)}`, () => {
      expect(() => parseThemePackage(JSON.stringify({ ...base, css }))).toThrow();
      expect(validateTheme({ ...base, css }).ok).toBe(false);
    });
  }
  it("still takes plain scoped css", () => {
    expect(() => parseThemePackage(JSON.stringify({ ...base, css: ".panel { border-radius: 2px; }" }))).not.toThrow();
  });
});
