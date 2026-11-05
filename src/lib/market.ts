// The marketplace: the index in this project's repository (marketplace/index.json), installed themes, which
// marketplace version is installed, and the share flow (a pre-filled GitHub issue). See docs/marketplace.md.
import { addons, marketFetch, marketGet, settings } from "./api";
import { compareVersions, validateIndex, validateTheme, type MarketEntry, type MarketIndex } from "./addon-rules.mjs";
import { installThemePackage, uninstallTheme, type ThemePackage } from "./theme";

export type { MarketEntry, MarketIndex } from "./addon-rules.mjs";

export const REPO = "https://github.com/imanimal/pocket-mods";
export const DOCS = {
  extensions: `${REPO}/blob/main/docs/extensions.md`,
  marketplace: `${REPO}/blob/main/docs/marketplace.md`,
  themes: `${REPO}/blob/main/docs/themes.md`,
  folder: `${REPO}/tree/main/marketplace`,
};
const RAW = "https://raw.githubusercontent.com/imanimal/pocket-mods/main/marketplace/";

export interface LoadedIndex { index: MarketIndex; source: "network" | "cache" | "offline" | "bundled"; problems: string[] }

/** The index. Entries that do not check out are left out (and listed in `problems`). */
export async function loadIndex(fresh = false): Promise<LoadedIndex> {
  const f = await marketFetch("index.json", fresh);
  let raw: unknown;
  try { raw = JSON.parse(f.text); } catch { throw new Error("Marketplace index isn't valid JSON"); }
  const r = validateIndex(raw);
  const ix = raw as MarketIndex;
  if (!ix || ix.format !== "pocket-mods.marketplace") throw new Error("Not a Pockymod marketplace index");
  // keep good entries even when another one is broken
  const bad = new Set(r.errors.map((e) => e.match(/^(themes|extensions)\[(\d+)\]/)?.slice(1, 3).join(":")).filter(Boolean));
  const keep = (key: "themes" | "extensions") => (Array.isArray(ix[key]) ? ix[key] : []).filter((_, i) => !bad.has(`${key}:${i}`));
  return { index: { ...ix, themes: keep("themes"), extensions: keep("extensions") }, source: f.source, problems: r.errors };
}

/** A package from the marketplace, checked to be the entry it claims. */
export async function fetchPackage(entry: MarketEntry): Promise<string> {
  const text = await marketGet(entry.path, true);
  let v: { id?: unknown; release?: unknown };
  try { v = JSON.parse(text); } catch { throw new Error(`${entry.name} on the marketplace is not valid JSON.`); }
  if (v.id !== entry.id) throw new Error(`${entry.name} on the marketplace has a different id inside (${String(v.id)}).`);
  return text;
}

export const screenshotUrl = (path: string) => `${RAW}${path}`;

// ── what is installed from the marketplace ──

const VERSIONS = "market.installed";
export async function marketVersions(): Promise<Record<string, string>> {
  try { return JSON.parse((await settings.get(VERSIONS)) || "{}"); } catch { return {}; }
}
export async function recordVersion(kind: "theme" | "extension", id: string, version: string | null) {
  const all = await marketVersions();
  if (version) all[`${kind}:${id}`] = version; else delete all[`${kind}:${id}`];
  await settings.set(VERSIONS, JSON.stringify(all));
}
/** The marketplace has a newer version than the one installed. */
export const isNewer = (entry: MarketEntry, installed: string | undefined) => !!installed && compareVersions(entry.version, installed) > 0;

// ── themes ──

export interface InstalledTheme { id: string; pkg: ThemePackage; text: string; release?: string }

export async function installedThemes(): Promise<InstalledTheme[]> {
  const texts = await addons.list("theme").catch(() => [] as string[]);
  const out: InstalledTheme[] = [];
  for (const text of texts) {
    try {
      const pkg = JSON.parse(text) as ThemePackage & { release?: string };
      if (pkg && typeof pkg.id === "string") out.push({ id: pkg.id, pkg, text, release: pkg.release });
    } catch { /* not a theme */ }
  }
  return out.sort((a, b) => a.pkg.name.localeCompare(b.pkg.name));
}

/** Check a theme's text the way the marketplace does. */
export function checkThemeText(text: string): { pkg: ThemePackage | null; errors: string[] } {
  let v: unknown;
  try { v = JSON.parse(text); } catch (e) { return { pkg: null, errors: [`Not valid JSON. ${e instanceof Error ? e.message : String(e)}`] }; }
  const r = validateTheme(v);
  return { pkg: r.ok ? (v as ThemePackage) : null, errors: r.errors };
}

const themesChanged = () => window.dispatchEvent(new Event("pm-themes"));

export async function installTheme(text: string, entry?: MarketEntry): Promise<{ id: string; name: string }> {
  const c = checkThemeText(text);
  if (!c.pkg) throw new Error(c.errors.slice(0, 3).join(" "));
  const r = await installThemePackage(text);
  const release = (c.pkg as ThemePackage & { release?: string }).release;
  await recordVersion("theme", r.id, entry?.version ?? release ?? null).catch(() => {});
  themesChanged();
  return r;
}
export async function removeTheme(id: string) {
  await uninstallTheme(id);
  await recordVersion("theme", id, null).catch(() => {});
  themesChanged();
}

/** Colours to draw a theme's card from its package. Missing tokens fall back to Cream or Night. */
export function themeColors(pkg: Partial<ThemePackage>, scheme: "dark" | "light" = pkg.scheme ?? "dark") {
  const base = scheme === "dark"
    ? { "--bg": "#1a1917", "--surface": "#262522", "--surface-2": "#302e2a", "--line": "#3f3c36", "--text": "#f4efe5", "--text-3": "#a39c8d", "--accent": "#ff6a2b", "--chrome-bg": "#0f0e0d", "--disp-bg": "#45b3a1", "--disp-fg": "#04211c" }
    : { "--bg": "#e6e2d8", "--surface": "#f6f3ec", "--surface-2": "#ebe7dd", "--line": "#d3ccbd", "--text": "#1b1915", "--text-3": "#6a645a", "--accent": "#ff5b1f", "--chrome-bg": "#2a2722", "--disp-bg": "#b1bd93", "--disp-fg": "#1f2a17" };
  const t: Record<string, string> = { ...base, ...(pkg.tokens ?? {}), ...((scheme === "dark" ? pkg.dark : pkg.light) ?? {}) };
  return { bg: t["--bg"], surface: t["--surface"], surface2: t["--surface-2"], line: t["--line"], text: t["--text"], text3: t["--text-3"], accent: t["--accent"], chrome: t["--chrome-bg"], display: t["--disp-bg"], displayFg: t["--disp-fg"] };
}
/** A theme's schemes: the one it starts in first. */
export const themeSchemes = (pkg: Partial<ThemePackage>): Array<"dark" | "light"> => {
  const both = pkg.dark && pkg.light;
  const first = pkg.scheme ?? "dark";
  return both ? [first, first === "dark" ? "light" : "dark"] : [first];
};

// ── sharing ──

const MAX_URL = 7500;

/** A GitHub "new issue" address with the submission form filled in. The package goes in when the address stays short
 *  enough for browsers and GitHub; otherwise the user attaches the file they saved. */
export function submissionUrl(kind: "theme" | "extension", name: string, text: string): { url: string; withPackage: boolean } {
  const label = kind === "theme" ? "Theme" : "Extension";
  const base = { template: "marketplace-submission.yml", title: `[Marketplace] ${label}: ${name}`, kind: label, name };
  const make = (pkg?: string) => `${REPO}/issues/new?${new URLSearchParams(pkg === undefined ? base : { ...base, package: pkg })}`;
  let compact = text;
  try { compact = JSON.stringify(JSON.parse(text)); } catch { /* send as is */ }
  for (const t of [text, compact]) { const url = make(t); if (url.length <= MAX_URL) return { url, withPackage: true }; }
  return { url: make(), withPackage: false };
}

/** A file name for an exported package. */
export const packageFileName = (kind: "theme" | "extension", id: string) => `${id}.${kind === "theme" ? "pmtheme" : "pmext"}.json`;
