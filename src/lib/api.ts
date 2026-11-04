// Typed calls to the Rust side. Shapes mirror pocket-core's Serialize output (camelCase).
import { call, listen } from "./tauri";

export interface ApiError { kind: string; message: string }
export const errorText = (e: unknown): string =>
  typeof e === "object" && e && "message" in e ? String((e as ApiError).message) : String(e);
export const errorKind = (e: unknown): string =>
  typeof e === "object" && e && "kind" in e ? String((e as ApiError).kind) : "unknown";

export type RateLevel = "unknown" | "ok" | "low" | "throttled";
export interface RateDisplay {
  level: RateLevel;
  remaining: number | null;
  limit: number | null;
  window: "hour" | "day";
  resetsAt: string | null;
  throttledUntil: string | null;
}

export interface NexusUser { userId: number; name: string; isPremium: boolean; isSupporter: boolean; profileUrl?: string | null }
export interface NexusStatus { hasKey: boolean; user: NexusUser | null; rateLimit: RateDisplay }

export const nexus = {
  status: (refresh = false) => call<NexusStatus>("nexus_status", { refresh }),
  setKey: (key: string) => call<NexusStatus>("nexus_set_key", { key }),
  clearKey: () => call<NexusStatus>("nexus_clear_key"),
  rateLimit: () => call<RateDisplay>("rate_limit"),
  onRateLimit: (cb: (r: RateDisplay) => void) => listen<RateDisplay>("rate-limit", cb),
};

// ── browser ──

export type FrameworkKey = "cet" | "red4ext" | "redscript" | "archivexl" | "tweakxl" | "codeware" | "cybercmd";
export type ModType = "redmod" | "archive" | "script" | "framework" | "mixed" | "unknown";
export type ModTypeFilter = "redmod" | "archive" | "script" | "framework";
export type SortKey = "downloads" | "endorsements" | "updated" | "created" | "relevance" | "size" | "name";

export const FRAMEWORKS: Array<{ key: FrameworkKey; name: string; short: string }> = [
  { key: "cet", name: "Cyber Engine Tweaks", short: "CET" },
  { key: "red4ext", name: "RED4ext", short: "RED4ext" },
  { key: "redscript", name: "redscript", short: "redscript" },
  { key: "archivexl", name: "ArchiveXL", short: "ArchiveXL" },
  { key: "tweakxl", name: "TweakXL", short: "TweakXL" },
  { key: "codeware", name: "Codeware", short: "Codeware" },
  { key: "cybercmd", name: "cybercmd", short: "cybercmd" },
];
export const MOD_TYPES: Array<{ key: ModTypeFilter; name: string }> = [
  { key: "redmod", name: "REDmod" }, { key: "archive", name: "Archive" }, { key: "script", name: "Script" }, { key: "framework", name: "Framework" },
];

export interface BrowseQuery {
  text?: string | null;
  categories: string[];
  tags: string[];
  excludeTags: string[];
  sort: SortKey;
  ascending: boolean;
  updatedAfter?: number | null;
  updatedBefore?: number | null;
  createdAfter?: number | null;
  createdBefore?: number | null;
  downloadsMin?: number | null;
  downloadsMax?: number | null;
  endorsementsMin?: number | null;
  sizeMinKb?: number | null;
  sizeMaxKb?: number | null;
  includeAdult: boolean;
  frameworks: FrameworkKey[];
  modType?: ModTypeFilter | null;
  compatibleSince?: number | null;
  offset: number;
  count: number;
}
export interface LocalFilter { showHidden: boolean; downloaded: boolean | null }

export interface FrameworkUse { framework: FrameworkKey; key: FrameworkKey; name: string; source: "files" | "requirements" | "tags" | "text" | "identity" }
export interface Classification { frameworks: FrameworkUse[]; modType: ModType; typeSource: string | null; isFramework: FrameworkKey | null }
export interface Requirement { modId: number; modName: string; externalRequirement: boolean; url?: string | null; notes?: string | null }
export interface ModCard {
  modId: number; uid: string; name: string; summary: string; author: string | null; uploader: { name: string; memberId: number } | null;
  thumbnailUrl: string | null; downloads: number; endorsements: number; fileSize: number | null; createdAt: string; updatedAt: string;
  version: string; category: string; adultContent: boolean | null; status: string; tags: Array<{ id: string; name: string }>;
  modRequirements?: { nexusRequirements: { totalCount: number; nodes: Requirement[] } };
}
export interface CardView extends ModCard { classification: Classification; hidden: boolean; ignored: boolean; downloaded: boolean }
export interface BrowseResult {
  total: number; offset: number; count: number; nodes: CardView[]; filteredOnPage: number;
  facets: { categories: Array<[string, number]>; tags: Array<[string, number]> };
  derived: Array<[string, number]>; fromCache: boolean;
}
export interface ModFile { fileId: number; name: string; version: string; category: string; size: number; sizeInBytes: number | null; date: number; primary: number; uri: string; description?: string | null }
export interface ModDetail extends Omit<ModCard, "thumbnailUrl" | "modRequirements"> {
  description: string; pictureUrl: string | null;
  modRequirements: { nexusRequirements: { totalCount: number; nodes: Requirement[] } };
}
export interface DetailView { detail: ModDetail; files: ModFile[]; classification: Classification; fileList: string[] | null; hidden: boolean; ignored: boolean; downloaded: boolean; gameId: number }
export interface Patch { version: string; released: string }

export const browse = {
  search: (query: BrowseQuery, local: LocalFilter) => call<BrowseResult>("browse_search", { query, local }),
  detail: (modId: number, updatedAt?: string) => call<DetailView>("browse_detail", { modId, updatedAt }),
  setFlag: (modId: number, flag: "hidden" | "ignored", on: boolean) => call<void>("set_mod_flag", { modId, flag, on }),
  patches: () => call<Array<[Patch, number | null]>>("patches"),
};
export const settings = {
  get: (key: string) => call<string | null>("settings_get", { key }),
  set: (key: string, value: string) => call<void>("settings_set", { key, value }),
};

export const fileBytes = (f: ModFile) => f.sizeInBytes ?? f.size * 1024;
