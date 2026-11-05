// Themes. A theme is a look (the shapes of keys, modules and LCDs: Standard, Redline or Pocket) plus a palette of
// tokens, in a dark scheme, a light one or both. Built-in themes are CSS (styles/themes/*.css, Pocket is
// pocket-os.css itself); installed ones are JSON packages (docs/themes.md) kept by the add-on store and compiled into
// <style id="pm-user-theme">. The result lives on <html>: data-theme (the theme's id), data-look, data-scheme
// (dark or light, plus .night when dark), data-fx (the CRT layer: off, subtle or full) and data-icons (pixel or line).
import { useSyncExternalStore } from "react";
import { flushSync } from "react-dom";
import { LOOKS, compileTheme, iconBytes, packageSchemes, parseThemePackage, type Look } from "./themePkg";

export type { Look } from "./themePkg";
export type Scheme = "dark" | "light";
export type SchemeChoice = Scheme | "system";
export type FxLevel = "off" | "subtle" | "full";
export type IconStyle = "pixel" | "line";
/** The first setting, from when Pocket was the only theme. Still read and written for older callers. */
export type ThemeChoice = "system" | "cream" | "night";

export interface ThemeInfo {
  id: string;
  name: string;
  author?: string;
  description?: string;
  builtin: boolean;
  look: Look;
  /** The schemes it has; the first is the one it starts in. */
  schemes: Scheme[];
  /** Compiled CSS (installed themes only). */
  css?: string;
  /** The package (installed themes only). */
  pkg?: ThemePackage;
  /** The package text as it was saved (installed themes only). */
  text?: string;
}

/** Redline, styled after the game's own HUD, is what a new install starts in. */
export const DEFAULT_THEME = "redline";

/** The default first: it is also where the app falls back to when a theme is removed. */
export const BUILTIN_THEMES: ThemeInfo[] = [
  { id: "redline", name: "Redline", builtin: true, look: "redline", schemes: ["dark"],
    description: "Red on black, cyan highlights. Dark only, default" },
  { id: "standard", name: "Standard", builtin: true, look: "standard", schemes: ["dark", "light"],
    description: "Blue-grey, yellow accent. Dark or light" },
  { id: "pocket", name: "Pocket", builtin: true, look: "pocket", schemes: ["light", "dark"],
    description: "Raised keys, green LCDs, pixel icons. Cream or Night" },
];

const K = { id: "pm.theme.id", scheme: "pm.theme.scheme", cache: "pm.theme.css", fx: "pm.fx", motion: "pm.motion", legacy: "pm.theme" };
const read = (k: string): string | null => { try { return localStorage.getItem(k); } catch { return null; } };
const write = (k: string, v: string | null) => {
  try { if (v === null) localStorage.removeItem(k); else localStorage.setItem(k, v); } catch { /* storage unavailable */ }
};

const media = () => window.matchMedia("(prefers-color-scheme: dark)");
const isFx = (v: unknown): v is FxLevel => v === "off" || v === "subtle" || v === "full";
const isChoice = (v: unknown): v is SchemeChoice => v === "dark" || v === "light" || v === "system";

export interface ThemeState {
  id: string;
  name: string;
  look: Look;
  /** What the user picked: dark, light or system. */
  choice: SchemeChoice;
  /** What is showing. */
  scheme: Scheme;
  schemes: Scheme[];
  /** The effects level showing, and the user's own pick (null follows the theme). */
  fx: FxLevel;
  fxChoice: FxLevel | null;
  /** The effects level the theme suggests. */
  fxTheme: FxLevel;
  icons: IconStyle;
  motion: boolean;
}

let installed = new Map<string, ThemeInfo>();
let state: ThemeState = {
  id: DEFAULT_THEME, name: "Redline", look: "redline", choice: "dark", scheme: "dark", schemes: ["dark"],
  fx: "full", fxChoice: null, fxTheme: "full", icons: "line", motion: true,
};
const subs = new Set<() => void>();

export const themeState = () => state;
export function subscribeTheme(cb: () => void): () => void { subs.add(cb); return () => { subs.delete(cb); }; }
/** The theme showing now, re-rendering when it changes. */
export const useTheme = () => useSyncExternalStore(subscribeTheme, themeState);

// ── what is saved ──

function savedId(): string {
  const id = read(K.id);
  if (id) return id;
  return read(K.legacy) ? "pocket" : DEFAULT_THEME; // someone who used the app before themes keeps Pocket
}

function savedChoice(): SchemeChoice | null {
  const c = read(K.scheme);
  if (isChoice(c)) return c;
  const old = read(K.legacy);
  if (old === "cream") return "light";
  if (old === "night") return "dark";
  if (old === "system") return "system";
  return null;
}

const savedFx = (): FxLevel | null => { const v = read(K.fx); return isFx(v) ? v : null; };

/** The installed theme's compiled CSS, kept so it shows from the first frame instead of after the add-on store answers. */
interface Cached { id: string; name: string; look: Look; schemes: Scheme[]; css: string }
function cachedInfo(id: string): ThemeInfo | null {
  try {
    const c = JSON.parse(read(K.cache) ?? "null") as Cached | null;
    if (c && c.id === id && typeof c.css === "string" && (LOOKS as readonly string[]).includes(c.look)) {
      return { id: c.id, name: c.name, look: c.look, schemes: c.schemes, css: c.css, builtin: false };
    }
  } catch { /* a broken cache is ignored */ }
  return null;
}
function cache(info: ThemeInfo | null) {
  if (!info || info.builtin || !info.css) { write(K.cache, null); return; }
  const c: Cached = { id: info.id, name: info.name, look: info.look, schemes: info.schemes, css: info.css };
  write(K.cache, JSON.stringify(c));
}

export function themeInfo(id: string): ThemeInfo | undefined {
  return BUILTIN_THEMES.find((t) => t.id === id) ?? installed.get(id) ?? cachedInfo(id) ?? undefined;
}

function resolveScheme(info: ThemeInfo, choice: SchemeChoice): Scheme {
  const want: Scheme = choice === "system" ? (media().matches ? "dark" : "light") : choice;
  return info.schemes.includes(want) ? want : info.schemes[0];
}

// ── painting ──

function motionOn(): boolean {
  const m = document.documentElement.dataset.motion;
  if (m === "0") return false;
  if (m === "1") return true;
  return !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function paint(info: ThemeInfo, choice: SchemeChoice) {
  const root = document.documentElement;
  const scheme = resolveScheme(info, choice);
  root.dataset.theme = info.id;
  root.dataset.look = info.look;
  root.dataset.scheme = scheme;
  root.classList.toggle("night", scheme === "dark");

  let st = document.getElementById("pm-user-theme") as HTMLStyleElement | null;
  if (info.css) {
    if (!st) { st = document.createElement("style"); st.id = "pm-user-theme"; }
    if (st.textContent !== info.css) st.textContent = info.css;
    if (st !== document.head.lastElementChild) document.head.appendChild(st); // last, so it wins ties with the looks
  } else st?.remove();

  // the theme says which effects and icons it wants through two tokens, --fx and --icons
  const cs = getComputedStyle(root);
  const fxRaw = cs.getPropertyValue("--fx").trim();
  const fxTheme: FxLevel = isFx(fxRaw) ? fxRaw : "off";
  const fxChoice = savedFx();
  const fx = fxChoice ?? fxTheme;
  root.dataset.fx = fx;
  const icons: IconStyle = cs.getPropertyValue("--icons").trim() === "line" ? "line" : "pixel";
  root.dataset.icons = icons;

  applyCustom();
  state = { id: info.id, name: info.name, look: info.look, choice, scheme, schemes: info.schemes, fx, fxChoice, fxTheme, icons, motion: motionOn() };
  subs.forEach((f) => f());
  appIcon(info);
}

/** The taskbar and tray wear the theme: its own icon, else its look's mark (src-tauri/src/icon.rs). */
let shownIcon = "";
function appIcon(info: ThemeInfo) {
  const own = info.pkg?.icon;
  const key = own ? `${info.id}:own` : info.look;
  if (key === shownIcon || !("__TAURI_INTERNALS__" in window)) return;
  shownIcon = key;
  void import("./tauri").then(({ call }) => call("app_icon", { look: info.look, png: own ? iconBytes(own) : null })).catch(() => { shownIcon = ""; });
}

/** Paint with a top-down redraw (a view transition) when motion is on, then tell the effects layer. */
function swap(fn: () => void, kind: "theme" | "scheme") {
  const root = document.documentElement;
  const done = () => window.dispatchEvent(new CustomEvent("pm-theme", { detail: kind }));
  const start = (document as Document & { startViewTransition?: (cb: () => void) => { finished: Promise<void> } }).startViewTransition;
  if (!start || !motionOn() || document.visibilityState === "hidden") { fn(); done(); return; }
  root.dataset.vt = "theme";
  const t = start.call(document, () => flushSync(fn));
  t.finished.catch(() => {}).finally(() => { delete root.dataset.vt; done(); });
}

function current(): ThemeInfo {
  return themeInfo(state.id) ?? BUILTIN_THEMES[0];
}

/** Paint the saved theme. Call once before the first render; installed themes come from the cache until loadInstalled(). */
export function initTheme(): ThemeState {
  const m = read(K.motion);
  if (m === "0" || m === "1") document.documentElement.dataset.motion = m;
  const info = themeInfo(savedId()) ?? BUILTIN_THEMES[0];
  paint(info, savedChoice() ?? info.schemes[0]);
  return state;
}

// ── installed themes ──

function infoFrom(text: string): ThemeInfo {
  const pkg = parseThemePackage(text);
  return {
    id: pkg.id, name: pkg.name, author: pkg.author, description: pkg.description, builtin: false,
    look: pkg.base ?? "standard", schemes: packageSchemes(pkg), css: compileTheme(pkg), pkg, text,
  };
}

/** Read the installed themes from the add-on store. A theme that no longer checks out is skipped. */
export async function loadInstalled(): Promise<ThemeInfo[]> {
  const { addons } = await import("./api");
  const texts = await addons.list("theme").catch(() => [] as string[]);
  const next = new Map<string, ThemeInfo>();
  for (const t of texts) { try { const i = infoFrom(t); next.set(i.id, i); } catch { /* skipped */ } }
  installed = next;
  const id = state.id;
  if (!BUILTIN_THEMES.some((t) => t.id === id)) {
    const info = installed.get(id);
    if (info) { cache(info); paint(info, state.choice); }
    else { write(K.id, null); cache(null); paint(BUILTIN_THEMES[0], savedChoice() ?? "dark"); } // it was removed
  }
  return [...installed.values()];
}

/** Built-in themes first, then the installed ones by name. */
export async function listThemes(): Promise<ThemeInfo[]> {
  const mine = await loadInstalled();
  return [...BUILTIN_THEMES, ...mine.sort((a, b) => a.name.localeCompare(b.name))];
}

/** Switch to a theme by id, optionally with a scheme. */
/** Switch a theme's own css off (and back on): the extension review shows exactly what it is, whatever a theme
 * would hide. */
export function suspendUserCss(on: boolean) {
  const st = document.getElementById("pm-user-theme") as HTMLStyleElement | null;
  if (st) st.media = on ? "not all" : "";
}

/** Back to the built-in default (Redline), from anywhere (Ctrl+Alt+Shift+T): the way out when a theme hides the app. */
export function resetTheme(): Promise<void> { return setTheme(DEFAULT_THEME, "dark"); }

export async function setTheme(id: string, choice?: SchemeChoice): Promise<void> {
  let info = themeInfo(id);
  if (!info || (!info.builtin && !installed.has(id))) { await loadInstalled(); info = installed.get(id) ?? BUILTIN_THEMES.find((t) => t.id === id); }
  if (!info) throw new Error("Theme not installed");
  const c = choice ?? savedChoice() ?? state.choice; // nothing saved yet: keep the scheme showing (dark for new users)
  write(K.id, info.id);
  if (choice) write(K.scheme, choice);
  cache(info);
  const changed = info.id !== state.id;
  const target = info;
  swap(() => paint(target, c), changed ? "theme" : "scheme");
}

/** Dark, light or system, within the current theme. */
export function setSchemeChoice(choice: SchemeChoice) {
  write(K.scheme, choice);
  const info = current();
  if (resolveScheme(info, choice) === state.scheme) paint(info, choice);
  else swap(() => paint(info, choice), "scheme");
}

/** Flip between dark and light. False when the theme has only one scheme. */
export function toggleScheme(): boolean {
  if (state.schemes.length < 2) return false;
  setSchemeChoice(state.scheme === "dark" ? "light" : "dark");
  return true;
}

/** The effects level; null goes back to what the theme suggests. */
export function setFx(level: FxLevel | null) {
  write(K.fx, level);
  paint(current(), state.choice);
  if (level && level !== "off") window.dispatchEvent(new CustomEvent("pm-theme", { detail: "fx" }));
}

export function getMotion(): boolean { return document.documentElement.dataset.motion !== "0"; }
export function setMotion(on: boolean) {
  document.documentElement.dataset.motion = on ? "1" : "0";
  write(K.motion, on ? "1" : "0");
  state = { ...state, motion: motionOn() };
  subs.forEach((f) => f());
}

/** Re-apply when the system setting changes while the choice is "system". */
export function watchSystemTheme(onChange: () => void): () => void {
  const m = media();
  const h = () => { if (state.choice === "system") { paint(current(), "system"); onChange(); } };
  m.addEventListener("change", h);
  return () => m.removeEventListener("change", h);
}

// ── older callers ──

export function getThemeChoice(): ThemeChoice {
  return state.choice === "system" ? "system" : state.choice === "dark" ? "night" : "cream";
}
/** Paint the current theme (optionally with a Cream/Night/System choice, not saved). Returns "night" when dark. */
export function applyTheme(choice?: ThemeChoice): "cream" | "night" {
  if (choice) paint(current(), choice === "system" ? "system" : choice === "night" ? "dark" : "light");
  else paint(current(), state.choice);
  return state.scheme === "dark" ? "night" : "cream";
}
export function setThemeChoice(choice: ThemeChoice) {
  setSchemeChoice(choice === "system" ? "system" : choice === "night" ? "dark" : "light");
}

// ── tokens: reading a theme's values, for previews, the theme maker and export ──

/** The colour tokens the theme maker edits, in the order it shows them. */
export const COLOR_TOKENS: Array<{ token: string; label: string }> = [
  { token: "--bg", label: "Window" },
  { token: "--surface", label: "Modules" },
  { token: "--surface-2", label: "Raised parts" },
  { token: "--bg2", label: "Fields" },
  { token: "--line", label: "Lines" },
  { token: "--text", label: "Titles" },
  { token: "--text-2", label: "Body text" },
  { token: "--text-3", label: "Quiet text" },
  { token: "--accent", label: "Accent" },
  { token: "--on-accent", label: "Text on accent" },
  { token: "--acc2", label: "Second accent" },
  { token: "--chrome-bg", label: "Bands" },
  { token: "--chrome-fg", label: "Band text" },
  { token: "--disp-bg", label: "LCD" },
  { token: "--disp-fg", label: "LCD digits" },
  { token: "--disp-hot", label: "LCD amber" },
  { token: "--disp-bad", label: "LCD red" },
  { token: "--ok", label: "Good" },
  { token: "--warn", label: "Warning" },
  { token: "--bad", label: "Problem" },
];
/** Everything an exported theme carries besides the colours. */
const SHAPE_TOKENS = ["--on-bad", "--radius", "--btn-r", "--win-r"];

/** A theme's token values in a scheme, read from a hidden sample of its look with the package's own tokens on top. */
export function readTokens(info: ThemeInfo, scheme: Scheme, names: string[] = COLOR_TOKENS.map((t) => t.token)): Record<string, string> {
  const root = document.documentElement;
  const live = info.id === state.id && scheme === state.scheme && !document.getElementById("pm-theme-draft");
  let cs: CSSStyleDeclaration;
  let el: HTMLElement | null = null;
  if (live) cs = getComputedStyle(root);
  else {
    el = document.createElement("div");
    el.className = "tprev";
    el.dataset.look = info.look;
    el.dataset.scheme = scheme;
    el.hidden = true;
    const own = { ...(info.pkg?.tokens ?? {}), ...((scheme === "dark" ? info.pkg?.dark : info.pkg?.light) ?? {}) };
    for (const [k, v] of Object.entries(own)) el.style.setProperty(k, v);
    document.body.appendChild(el);
    cs = getComputedStyle(el);
  }
  const out: Record<string, string> = {};
  for (const n of names) out[n] = cs.getPropertyValue(n).trim();
  el?.remove();
  return out;
}

/** Any CSS colour as #rrggbb (for colour inputs). Transparent parts are dropped. */
export function toHex(value: string): string {
  if (/^#[0-9a-f]{6}$/i.test(value)) return value.toLowerCase();
  if (/^#[0-9a-f]{3}$/i.test(value)) return `#${value.slice(1).split("").map((c) => c + c).join("")}`.toLowerCase();
  const el = document.createElement("span");
  el.style.color = value;
  el.hidden = true;
  document.body.appendChild(el);
  const c = getComputedStyle(el).color;
  el.remove();
  const hex = (n: number) => Math.round(Math.max(0, Math.min(255, n))).toString(16).padStart(2, "0");
  let m = /rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)/.exec(c);
  if (m) return `#${hex(+m[1])}${hex(+m[2])}${hex(+m[3])}`;
  m = /color\(srgb\s+([\d.e-]+)\s+([\d.e-]+)\s+([\d.e-]+)/.exec(c);
  if (m) return `#${hex(+m[1] * 255)}${hex(+m[2] * 255)}${hex(+m[3] * 255)}`;
  return "#808080";
}

/** A theme as a package, to export or to start a new one from. Built-in themes come out as a copy that builds on them. */
export function themeAsPackage(info: ThemeInfo): ThemePackage {
  if (info.pkg) return info.pkg;
  const names = [...COLOR_TOKENS.map((t) => t.token), ...SHAPE_TOKENS];
  const pkg: ThemePackage = {
    format: "pocket-mods.theme", version: 1, id: `my-${info.id}`, name: `My ${info.name}`,
    description: `Based on ${info.name}`, scheme: info.schemes[0], base: info.look,
  };
  if (info.schemes.length > 1) { pkg.dark = readTokens(info, "dark", names); pkg.light = readTokens(info, "light", names); }
  else pkg.tokens = readTokens(info, info.schemes[0], names);
  return pkg;
}

/** The current theme as JSON text and a file name. */
export function exportTheme(id = state.id): { file: string; text: string } {
  const info = themeInfo(id) ?? current();
  const pkg = themeAsPackage(info);
  return { file: `${pkg.id}.json`, text: info.text && !info.builtin ? info.text : JSON.stringify(pkg, null, 2) + "\n" };
}

/** Show draft tokens over the whole app while a theme is being made; null takes the draft away. */
// ── your colours on any theme ──
// Colour changes kept per theme and scheme, layered over the theme itself (built-in or installed), so tweaking a
// theme never means making a new one. Kept in local storage; Reset takes them off.

const CUSTOM = "pm.theme.custom";
export type Custom = Partial<Record<Scheme, Record<string, string>>>;
function allCustom(): Record<string, Custom> { try { return JSON.parse(read(CUSTOM) ?? "{}") ?? {}; } catch { return {}; } }
export const customFor = (id: string): Custom => allCustom()[id] ?? {};
/** Set (or with null, clear) your colours for a theme in a scheme, and show them. */
export function setCustom(id: string, scheme: Scheme, tokens: Record<string, string> | null) {
  const all = allCustom();
  const c: Custom = { ...(all[id] ?? {}) };
  if (tokens && Object.keys(tokens).length) c[scheme] = tokens; else delete c[scheme];
  if (Object.keys(c).length) all[id] = c; else delete all[id];
  write(CUSTOM, Object.keys(all).length ? JSON.stringify(all) : null);
  applyCustom();
  state = { ...state }; // a new snapshot, so screens showing the colours draw again
  subs.forEach((f) => f());
}
const safeTok = (k: string) => /^--[a-zA-Z0-9_-]+$/.test(k);
const safeVal = (v: string) => String(v).replace(/[;{}<>\\]/g, "").replace(/url\(|@import|image-set/gi, "");
/** The style for every theme's own colours; it beats the themes (the :not(#_) adds weight), not a draft. */
export function applyCustom() {
  let st = document.getElementById("pm-theme-custom") as HTMLStyleElement | null;
  const css = Object.entries(allCustom()).flatMap(([id, c]) => (["dark", "light"] as Scheme[]).map((s) => {
    const t = c[s];
    if (!t || !/^[a-z0-9.-]+$/.test(id)) return "";
    const body = Object.entries(t).filter(([k]) => safeTok(k)).map(([k, v]) => `${k}: ${safeVal(v)};`).join(" ");
    return `html:root[data-theme="${id}"][data-scheme="${s}"]:not(#_) { ${body} }`;
  })).filter(Boolean).join("\n");
  if (!css) { st?.remove(); return; }
  if (!st) { st = document.createElement("style"); st.id = "pm-theme-custom"; }
  st.textContent = css;
  const draft = document.getElementById("pm-theme-draft");
  if (draft) document.head.insertBefore(st, draft); else document.head.appendChild(st);
}
/** The few colours most people want, and the shades worked out from them. */
export const SIMPLE: Array<{ key: string; label: string; tip: string; tokens: (v: string) => Record<string, string> }> = [
  { key: "--accent", label: "Accent", tip: "Main keys like Install and Play", tokens: (v) => ({ "--accent": v }) },
  { key: "--acc2", label: "Highlight", tip: "Selection, focus and links", tokens: (v) => ({ "--acc2": v, "--sel": v, "--focus": v }) },
  { key: "--bg", label: "Background", tip: "Window background, other shades follow", tokens: (v) => ({
    "--bg": v, "--bg2": `color-mix(in srgb, ${v} 88%, var(--text))`, "--surface": `color-mix(in srgb, ${v} 93%, var(--text))`,
    "--surface-2": `color-mix(in srgb, ${v} 86%, var(--text))`, "--line": `color-mix(in srgb, ${v} 72%, var(--text))`,
  }) },
  { key: "--text", label: "Text", tip: "Titles, other text shades follow", tokens: (v) => ({
    "--text": v, "--text-2": `color-mix(in srgb, ${v} 80%, var(--bg))`, "--text-3": `color-mix(in srgb, ${v} 60%, var(--bg))`,
  }) },
  { key: "--disp-bg", label: "Displays", tip: "LCD number displays", tokens: (v) => ({ "--disp-bg": v }) },
];
/** One-click accent pairs. */
export const PALETTES: Array<{ name: string; accent: string; acc2: string }> = [
  { name: "Signal red", accent: "#ff4f4f", acc2: "#45f0ff" }, { name: "Hazard yellow", accent: "#f5d742", acc2: "#4fd6df" },
  { name: "Toxic green", accent: "#7dff4f", acc2: "#ff4fd8" }, { name: "Ice blue", accent: "#4fb6ff", acc2: "#f5f5f5" },
  { name: "Violet", accent: "#a56bff", acc2: "#ffd24f" }, { name: "Orange", accent: "#ff6a2b", acc2: "#2f6fde" },
];

export function previewDraft(maps: Partial<Record<Scheme, Record<string, string>>> | null) {
  let st = document.getElementById("pm-theme-draft") as HTMLStyleElement | null;
  if (!maps) { st?.remove(); return; }
  if (!st) { st = document.createElement("style"); st.id = "pm-theme-draft"; }
  const block = (s: Scheme) => {
    const t = maps[s];
    if (!t) return "";
    const body = Object.entries(t).filter(([k]) => /^--[a-zA-Z0-9_-]+$/.test(k)).map(([k, v]) => `${k}: ${String(v).replace(/[;{}<>\\]/g, "")};`).join(" ");
    return `html:root:root[data-theme][data-scheme="${s}"] { ${body} }`;
  };
  st.textContent = `${block("dark")}\n${block("light")}`;
  document.head.appendChild(st);
}

/** Show a scheme without saving it (the theme maker's Dark/Light switch). */
export function previewScheme(scheme: Scheme | null) {
  const info = current();
  paint(info, scheme ?? (savedChoice() ?? info.schemes[0]));
}

// ── installed themes (shared contract; the marketplace uses these) ──

/** A theme package as a user writes, imports or gets from the marketplace. See docs/themes.md. */
export interface ThemePackage {
  format: "pocket-mods.theme";
  version: 1;
  id: string;
  name: string;
  author?: string;
  description?: string;
  /** The scheme it starts in; a theme with both `dark` and `light` follows the Light/Dark choice. */
  scheme: "dark" | "light";
  /** The built-in look it builds on (standard, redline or pocket); standard when left out. */
  base?: Look;
  /** CSS custom properties, for example { "--bg": "#101014" }, used in both schemes. */
  tokens?: Record<string, string>;
  dark?: Record<string, string>;
  light?: Record<string, string>;
  /** Extra CSS, scoped by the app to html[data-theme="<id>"]. */
  css?: string;
  /** The app's icon (taskbar, tray) while the theme is on: a PNG as a data address. Else the base look's mark. */
  icon?: string;
}

/** Validate and save a theme package (JSON text). Returns its id and name. */
export async function installThemePackage(text: string): Promise<{ id: string; name: string }> {
  const info = infoFrom(text); // throws with a message that says what to fix
  const { addons } = await import("./api");
  await addons.save("theme", info.id, text);
  installed.set(info.id, info);
  if (state.id === info.id) { cache(info); paint(info, state.choice); }
  else { state = { ...state }; subs.forEach((f) => f()); } // the theme lists show it at once
  return { id: info.id, name: info.name };
}

export async function uninstallTheme(id: string): Promise<void> {
  const { addons } = await import("./api");
  await addons.remove("theme", id);
  installed.delete(id);
  if (state.id === id) { write(K.id, null); cache(null); swap(() => paint(BUILTIN_THEMES[0], savedChoice() ?? "dark"), "theme"); }
  else { state = { ...state }; subs.forEach((f) => f()); }
}
