// Types for addon-rules.mjs. The manifest shape is documented for authors in docs/extensions.md.

export const EXTENSION_FORMAT: "pocket-mods.extension";
export const THEME_FORMAT: "pocket-mods.theme";
export const MARKET_FORMAT: "pocket-mods.marketplace";

export type Permission = "library.read" | "library.toggle" | "nexus.search" | "queue.add" | "toast" | "nexus.open" | "storage";
export const PERMISSIONS: Record<Permission, { title: string; detail: string }>;
export type ContributionKey = "shelves" | "groupRules" | "tools" | "launchOptions" | "commandPacks" | "links" | "panels";
export const CONTRIBUTIONS: ContributionKey[];
export const SORT_KEYS: string[];
export const MOD_TYPES: string[];
export const ALL_MOD_TYPES: string[];
export const FRAMEWORK_KEYS: string[];
export const PREVIEW_KEYS: string[];
export const LIMITS: { panelHtml: number; manifest: number; storage: number; perList: number; commands: number; args: number };

/** A built-in icon name, or nine rows of nine "." and "#" characters. */
export type AddonIcon = string | string[];

export interface ShelfQuery {
  text?: string; categories?: string[]; tags?: string[]; excludeTags?: string[];
  sort?: "downloads" | "endorsements" | "updated" | "created" | "relevance" | "size" | "name" | "random"; ascending?: boolean;
  frameworks?: Array<"cet" | "red4ext" | "redscript" | "archivexl" | "tweakxl" | "codeware" | "cybercmd">;
  modType?: "redmod" | "archive" | "script" | "framework";
  endorsementsMin?: number; downloadsMax?: number;
  /** Only mods first released in the last N days. */
  createdWithinDays?: number;
}
export interface ShelfContribution { id: string; title: string; hint?: string; icon?: string; query: ShelfQuery }
export interface GroupRule {
  group: string;
  match: { name?: string[]; category?: string[]; author?: string[]; modType?: string[]; frameworks?: string[]; mods?: number[] };
}
export interface ToolContribution {
  id: string; name: string; description?: string;
  /** A program inside the game folder, like tools/redmod/bin/redMod.exe. */
  path?: string;
  /** The user picks the program (once, on install or later). `file` is the file name it must have. */
  pick?: { title?: string; file?: string };
  /** Arguments; {game} becomes the game folder. */
  args?: string[];
  /** Where to get the program. */
  download?: string;
}
export interface LaunchOption { id: string; name: string; description?: string; args: string[] }
export interface CommandPack { id: string; title: string; description?: string; mods?: number[]; commands: Array<{ command: string; label?: string }> }
export interface LinkContribution { id: string; title: string; url: string }
export interface PanelContribution { id: string; title: string; icon?: string; html: string }

export interface ExtensionManifest {
  format: "pocket-mods.extension";
  version: 1;
  id: string;
  name: string;
  release: string;
  author?: string;
  description?: string;
  homepage?: string;
  icon?: AddonIcon;
  tags?: string[];
  permissions?: Permission[];
  contributes: {
    shelves?: ShelfContribution[];
    groupRules?: GroupRule[];
    tools?: ToolContribution[];
    launchOptions?: LaunchOption[];
    commandPacks?: CommandPack[];
    links?: LinkContribution[];
    panels?: PanelContribution[];
  };
}

export interface MarketEntry {
  id: string;
  kind: "theme" | "extension";
  name: string;
  author: string;
  version: string;
  description: string;
  tags?: string[];
  /** The package file, relative to marketplace/. */
  path: string;
  /** YYYY-MM-DD. */
  updated: string;
  /** Theme colours for the card, so it draws without fetching the package. */
  preview?: Partial<Record<"bg" | "surface" | "accent" | "text" | "chrome" | "display", string>>;
  icon?: AddonIcon;
  /** An image in the marketplace folder. */
  screenshot?: string;
  permissions?: Permission[];
  /** What it adds, in words ("4 Browse shelves"). */
  adds?: string[];
}
export interface MarketIndex { format: "pocket-mods.marketplace"; version: 1; updated?: string; themes: MarketEntry[]; extensions: MarketEntry[] }

export interface Checked { ok: boolean; errors: string[]; warnings: string[] }
export function isId(s: unknown): boolean;
export function isSemver(s: unknown): boolean;
export function compareVersions(a: string, b: string): number;
export function validateExtension(m: unknown): Checked;
export function validateTheme(t: unknown): Checked;
export function validateIndex(ix: unknown): Checked;
export function approvalItems(m: ExtensionManifest): string[];
export function summarize(m: ExtensionManifest): string[];
