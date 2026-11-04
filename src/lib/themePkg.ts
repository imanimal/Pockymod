// Theme packages: checking a package someone wrote, imported or got from the marketplace, and turning it into CSS.
// Pure functions with no DOM, so they run in tests. docs/themes.md describes the format and these rules for authors.
import type { ThemePackage } from "./theme";

/** The built-in looks a theme can build on: the shapes of keys, modules and LCDs. Their ids are reserved. */
export const LOOKS = ["standard", "redline", "pocket"] as const;
export type Look = (typeof LOOKS)[number];

export const LIMITS = { id: 64, name: 60, author: 80, description: 300, tokens: 200, value: 400, css: 64 * 1024 };

/** Tokens a theme may not set: the motion multiplier belongs to the user's motion setting, --press to each key. */
const RESERVED = new Set(["--m", "--press", "--i", "--p"]);
/** Functions that can fetch something or run code. url() is checked separately: only data:image is kept. */
const BLOCKED_FNS = new Set(["expression", "image-set", "-webkit-image-set", "src", "image", "cross-fade", "-webkit-cross-fade", "element", "-moz-element"]);
const DATA_IMAGE = /^data:image\/(png|jpe?g|gif|webp|avif|svg\+xml)[;,]/i;

export class ThemeError extends Error {}
const fail = (msg: string): never => { throw new ThemeError(msg); };

export function checkId(id: unknown): string {
  if (typeof id !== "string" || !id) fail("The theme needs an id, for example my-theme.");
  const s = id as string;
  if (s.length > LIMITS.id || !/^[a-z0-9][a-z0-9.-]*$/.test(s) || s.includes("..")) {
    fail("A theme id uses lowercase letters, digits, dots and dashes, starts with a letter or digit, and is at most 64 long (for example my-theme).");
  }
  if ((LOOKS as readonly string[]).includes(s)) fail(`The id ${s} belongs to a built-in theme. Pick another one.`);
  return s;
}

export function checkTokenName(name: string): string {
  if (!/^--[a-zA-Z0-9_-]{1,62}$/.test(name)) fail(`${name || "A token"} is not a CSS custom property. Token names start with -- (for example --bg).`);
  if (RESERVED.has(name)) fail(`${name} is set by the app, not by themes.`);
  return name;
}

/** Walk a piece of CSS, calling `fn` for each function call outside strings with its name and raw argument text. */
function eachFunction(src: string, fn: (name: string, arg: string, start: number, end: number) => void) {
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (c === '"' || c === "'") { i = skipString(src, i); continue; }
    if (/[a-zA-Z_-]/.test(c) && (i === 0 || !/[a-zA-Z0-9_-]/.test(src[i - 1]))) {
      let j = i;
      while (j < src.length && /[a-zA-Z0-9_-]/.test(src[j])) j++;
      if (src[j] === "(") {
        const close = matchParen(src, j);
        fn(src.slice(i, j).toLowerCase(), src.slice(j + 1, close), i, close + 1);
        i = j + 1;
        continue;
      }
      i = j;
      continue;
    }
    i++;
  }
}

function skipString(src: string, i: number): number {
  const q = src[i];
  let j = i + 1;
  while (j < src.length && src[j] !== q) {
    if (src[j] === "\\") j++;
    // CSS ends a line, and so an unclosed string, at \n, \r and \f alike
    else if (src[j] === "\n" || src[j] === "\r" || src[j] === "\f") fail("A string in the theme is not closed.");
    j++;
  }
  if (j >= src.length) fail("A string in the theme is not closed.");
  return j + 1;
}

function matchParen(src: string, open: number): number {
  let depth = 0;
  for (let i = open; i < src.length; i++) {
    const c = src[i];
    if (c === '"' || c === "'") { i = skipString(src, i) - 1; continue; }
    if (c === "(") depth++;
    else if (c === ")") { depth--; if (depth === 0) return i; }
  }
  return fail("A ( in the theme is not closed.");
}

const unquote = (s: string) => s.trim().replace(/^(["'])([\s\S]*)\1$/, "$2").trim();

/** Check the functions in a value or a block of CSS. With `strip`, external url()s become `none` instead of failing. */
function checkFunctions(src: string, where: string, strip: boolean): string {
  const cuts: Array<[number, number]> = [];
  eachFunction(src, (name, arg, start, end) => {
    if (BLOCKED_FNS.has(name)) fail(`${where}: ${name}() is not allowed in themes.`);
    if (name === "url") {
      const a = unquote(arg);
      if (a.includes("\\")) fail(`${where}: a url() cannot use escapes.`);
      if (!DATA_IMAGE.test(a)) { if (strip) cuts.push([start, end]); else fail(`${where}: only data:image urls are allowed, nothing from the web or from files.`); }
    }
  });
  let out = src;
  for (const [s, e] of cuts.reverse()) out = out.slice(0, s) + "none" + out.slice(e);
  return out;
}

/** A value with its strings and url() contents emptied, so what is left is plain syntax to check. */
function masked(s: string): string {
  let out = "";
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === '"' || c === "'") { i = skipString(s, i) - 1; out += '""'; continue; }
    if (c === "(" && /url$/i.test(out)) { i = matchParen(s, i); out += "()"; continue; }
    out += c;
  }
  return out;
}

/** What a theme's css may never contain, whatever the parser below makes of it: escapes and the other CSS line
 * breaks (the ways to make it see something else than the browser does), and anything that loads or runs. */
const CSS_NEVER = /url\s*\(|image-set|\bimage\s*\(|cross-fade|element\s*\(|expression\s*\(|@import|@font-face|@namespace|-moz-binding|behavior\s*:/i;
export function checkCssText(css: string): void {
  if (/[\\\r\f\0]/.test(css)) fail("css: backslashes, carriage returns and form feeds are not allowed. Write characters as they are.");
  if (CSS_NEVER.test(css)) fail("css: it cannot load anything (no url(), image-set(), @import or @font-face). Put pictures in a token as data:image instead.");
}

/** A token value: one CSS value, no way to end the declaration or reach outside the theme. */
export function checkValue(name: string, v: unknown): string {
  if (typeof v !== "string") return fail(`${name}: the value must be text, for example "#101014".`);
  const s = v.trim();
  if (!s) fail(`${name} has no value.`);
  if (s.length > LIMITS.value) fail(`${name}: the value is too long.`);
  if (/[{}\\\r\f\0]/.test(s)) fail(`${name}: a value cannot contain { } \\ or line breaks.`);
  if (/javascript:/i.test(s)) fail(`${name}: that value is not allowed.`);
  const bare = masked(s);
  if (/[;<>!]/.test(bare)) fail(`${name}: a value cannot contain ; < > or ! (outside a string).`);
  if (bare.includes("/*")) fail(`${name}: a value cannot contain comments.`);
  let depth = 0;
  for (const c of bare) {
    if (c === "(") depth++;
    else if (c === ")" && --depth < 0) fail(`${name}: the brackets do not match.`);
  }
  if (depth !== 0) fail(`${name}: the brackets do not match.`);
  checkFunctions(s, name, false);
  return s;
}

function checkTokens(obj: unknown, field: string): Record<string, string> | undefined {
  if (obj === undefined || obj === null) return undefined;
  if (typeof obj !== "object" || Array.isArray(obj)) return fail(`${field} must be an object of tokens, for example { "--bg": "#101014" }.`);
  const out: Record<string, string> = {};
  const entries = Object.entries(obj as Record<string, unknown>);
  if (entries.length > LIMITS.tokens) fail(`${field} has more than ${LIMITS.tokens} tokens.`);
  for (const [k, v] of entries) out[checkTokenName(k)] = checkValue(k, v);
  return out;
}

const optText = (v: unknown, field: string, max: number): string | undefined => {
  if (v === undefined || v === null) return undefined;
  if (typeof v !== "string") return fail(`${field} must be text.`);
  if (v.length > max) fail(`${field} is longer than ${max} characters.`);
  return v.trim() || undefined;
};

/** Parse and check a theme package. Throws a ThemeError whose message says what to fix. */
export function parseThemePackage(text: string): ThemePackage {
  let raw: unknown;
  try { raw = JSON.parse(text); } catch { return fail("That file is not valid JSON."); }
  return checkThemePackage(raw);
}

export function checkThemePackage(raw: unknown): ThemePackage {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) fail("That is not a Pockymod theme.");
  const p = raw as Record<string, unknown>;
  if (p.format !== "pocket-mods.theme") fail('That is not a Pockymod theme: "format" must be "pocket-mods.theme".');
  if (p.version !== 1) fail('This app reads theme version 1: set "version": 1.');
  const id = checkId(p.id);
  const name = optText(p.name, "name", LIMITS.name);
  if (!name) fail("The theme needs a name.");
  if (p.scheme !== "dark" && p.scheme !== "light") fail('"scheme" must be "dark" or "light".');
  if (p.base !== undefined && !(LOOKS as readonly unknown[]).includes(p.base)) fail(`"base" must be one of ${LOOKS.join(", ")}.`);
  if (p.css !== undefined && typeof p.css !== "string") fail('"css" must be text.');
  const icon = checkIcon(p.icon);
  const pkg: ThemePackage = {
    format: "pocket-mods.theme", version: 1, id, name: name!,
    author: optText(p.author, "author", LIMITS.author),
    description: optText(p.description, "description", LIMITS.description),
    scheme: p.scheme as "dark" | "light",
    base: p.base as Look | undefined,
    tokens: checkTokens(p.tokens, "tokens"),
    dark: checkTokens(p.dark, "dark"),
    light: checkTokens(p.light, "light"),
    css: p.css as string | undefined,
    icon,
  };
  if (pkg.css) { checkCssText(pkg.css); scopeCss(pkg.css, id); } // throws if it cannot be used
  for (const k of Object.keys(pkg) as Array<keyof ThemePackage>) if (pkg[k] === undefined) delete pkg[k];
  return pkg;
}

/** A theme's own app icon: a PNG as a data address, at most 256 KB (Rust checks its size in pixels too). */
function checkIcon(v: unknown): string | undefined {
  if (v === undefined) return undefined;
  if (typeof v !== "string" || !/^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(v)) fail('"icon" must be a PNG as a data address (data:image/png;base64,…).');
  if ((v as string).length > 350_000) fail('"icon" is too large: keep the PNG under 256 KB.');
  return v as string;
}

/** The bytes of a theme icon's PNG. */
export function iconBytes(dataUrl: string): number[] {
  const b = atob(dataUrl.slice(dataUrl.indexOf(",") + 1));
  return Array.from(b, (c) => c.charCodeAt(0));
}

/** The schemes a package has: both when it sets both `dark` and `light`, else the one it names. */
export const packageSchemes = (p: ThemePackage): Array<"dark" | "light"> => (p.dark && p.light ? [p.scheme, p.scheme === "dark" ? "light" : "dark"] : [p.scheme]);

const decls = (t: Record<string, string>) => Object.entries(t).map(([k, v]) => `  ${k}: ${v};`).join("\n");

/** The CSS for a checked package. Tokens beat the built-in looks (html:root is one step more specific than theirs). */
export function compileTheme(p: ThemePackage): string {
  const at = (extra: string) => `html:root[data-theme="${p.id}"]${extra}`;
  const out = [`/* theme: ${p.id} */`];
  if (p.tokens && Object.keys(p.tokens).length) out.push(`${at("[data-scheme]")} {\n${decls(p.tokens)}\n}`);
  if (p.dark && Object.keys(p.dark).length) out.push(`${at('[data-scheme="dark"]')} {\n${decls(p.dark)}\n}`);
  if (p.light && Object.keys(p.light).length) out.push(`${at('[data-scheme="light"]')} {\n${decls(p.light)}\n}`);
  if (p.css) out.push(scopeCss(p.css, p.id));
  return out.join("\n");
}

// ── scoping a theme's own CSS ──

type Node = { kind: "rule"; sel: string; body: string } | { kind: "at"; name: string; prelude: string; body: string | null };

function stripComments(src: string): string {
  let out = "";
  for (let i = 0; i < src.length; i++) {
    const c = src[i];
    if (c === '"' || c === "'") { const j = skipString(src, i); out += src.slice(i, j); i = j - 1; continue; }
    if (c === "/" && src[i + 1] === "*") {
      const e = src.indexOf("*/", i + 2);
      if (e < 0) fail("A comment in the css is not closed.");
      out += " ";
      i = e + 1;
      continue;
    }
    out += c;
  }
  return out;
}

/** Index of the } that closes the { at `open`. */
function matchBrace(src: string, open: number): number {
  let depth = 0;
  for (let i = open; i < src.length; i++) {
    const c = src[i];
    if (c === '"' || c === "'") { i = skipString(src, i) - 1; continue; }
    if (c === "{") depth++;
    else if (c === "}") { depth--; if (depth === 0) return i; }
  }
  return fail("The css has a { without its }.");
}

function parseBlock(src: string): Node[] {
  const nodes: Node[] = [];
  let i = 0;
  while (i < src.length) {
    while (i < src.length && /\s|;/.test(src[i])) i++;
    if (i >= src.length) break;
    if (src[i] === "}") fail("The css has a } without its {.");
    // read up to the first top-level { or ;
    let j = i, paren = 0;
    while (j < src.length) {
      const c = src[j];
      if (c === '"' || c === "'") { j = skipString(src, j); continue; }
      if (c === "(") paren++;
      else if (c === ")") paren--;
      else if (paren <= 0 && (c === "{" || c === ";" || c === "}")) break;
      j++;
    }
    const head = src.slice(i, j).trim();
    if (src[j] === "}") fail("The css has a } without its {.");
    if (head.startsWith("@")) {
      const m = /^@([a-zA-Z-]+)\s*([\s\S]*)$/.exec(head);
      const name = (m?.[1] ?? "").toLowerCase();
      const prelude = (m?.[2] ?? "").trim();
      if (j >= src.length || src[j] === ";") { nodes.push({ kind: "at", name, prelude, body: null }); i = j + 1; continue; }
      const close = matchBrace(src, j);
      nodes.push({ kind: "at", name, prelude, body: src.slice(j + 1, close) });
      i = close + 1;
      continue;
    }
    if (j >= src.length || src[j] === ";") { i = j + 1; continue; } // a stray declaration at the top: dropped
    const close = matchBrace(src, j);
    nodes.push({ kind: "rule", sel: head, body: src.slice(j + 1, close) });
    i = close + 1;
  }
  return nodes;
}

/** Split a selector list on its top-level commas. */
function splitList(sel: string): string[] {
  const parts: string[] = [];
  let depth = 0, start = 0;
  for (let i = 0; i < sel.length; i++) {
    const c = sel[i];
    if (c === '"' || c === "'") { i = skipString(sel, i) - 1; continue; }
    if (c === "(" || c === "[") depth++;
    else if (c === ")" || c === "]") depth--;
    else if (c === "," && depth === 0) { parts.push(sel.slice(start, i)); start = i + 1; }
  }
  parts.push(sel.slice(start));
  return parts.map((s) => s.trim()).filter(Boolean);
}

/** Put one selector under html[data-theme="id"]: a leading :root, html or & becomes that, anything else goes inside it. */
export function scopeSelector(sel: string, id: string): string {
  const root = `html[data-theme="${id}"]`;
  const m = /^(:root|html|&)(?=$|[\s>+~.:#[])/i.exec(sel);
  if (m) return root + sel.slice(m[0].length);
  if (/^body(?=$|[\s>+~.:#[])/i.test(sel)) return `${root} ${sel}`;
  return `${root} ${sel}`;
}

const GROUP_RULES = new Set(["media", "supports", "container", "layer", "starting-style"]);

function emit(nodes: Node[], id: string, depth: number): string {
  const out: string[] = [];
  for (const n of nodes) {
    if (n.kind === "rule") {
      const sel = splitList(n.sel).map((s) => scopeSelector(s, id)).join(", ");
      if (!sel) continue;
      out.push(`${sel} {${checkFunctions(n.body, "css", true)}}`);
      continue;
    }
    if (n.body === null) continue; // @import, @charset, @namespace, @layer statements: dropped
    if (GROUP_RULES.has(n.name)) {
      out.push(`@${n.name} ${checkFunctions(n.prelude, "css", true)} {\n${emit(parseBlock(n.body), id, depth + 1)}\n}`);
    } else if ((n.name === "keyframes" || n.name === "-webkit-keyframes" || n.name === "property") && depth === 0) {
      if (!/^[a-zA-Z0-9_-]+$/.test(n.prelude)) fail(`css: @${n.name} needs a plain name.`);
      out.push(`@${n.name} ${n.prelude} {${checkFunctions(n.body, "css", true)}}`);
    }
    // anything else (@font-face, @page, @scope …) is dropped: fonts come from the app, and nothing reaches outside
  }
  return out.join("\n");
}

/**
 * Make a theme's `css` safe to load: comments, @import, @font-face and external url()s are removed, every selector is
 * scoped to html[data-theme="<id>"], and anything that could break out of that (an unmatched brace, an escape, a
 * script url, a blocked function) is refused with a message.
 */
export function scopeCss(css: string, id: string): string {
  if (css.length > LIMITS.css) fail(`css is longer than ${LIMITS.css / 1024} KB.`);
  const src = stripComments(css);
  for (let i = 0; i < src.length; i++) {
    const c = src[i];
    if (c === '"' || c === "'") { i = skipString(src, i) - 1; continue; }
    if (c === "\\") fail("css: escapes (\\) are only allowed inside strings.");
  }
  if (/javascript:|behavior\s*:|-moz-binding/i.test(src)) fail("css: that property or value is not allowed.");
  return emit(parseBlock(src), id, 0);
}
