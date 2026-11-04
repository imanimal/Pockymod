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

// ── library, queue, paths ──

export type ItemState =
  | { state: "waiting" } | { state: "needsClick" } | { state: "downloading"; received: number; total: number | null }
  | { state: "installing" } | { state: "done"; installId: number | null; note: string | null } | { state: "failed"; message: string };
export interface QueueItem {
  id: number; modId: number; fileId: number; modName: string; fileName: string; version: string; uri: string; size: number;
  requiredBy: string | null; manualOnly: boolean; state: ItemState;
}
export interface QueueSummary {
  left: number; needsClick: number; bytesLeft: number; running: boolean; waitingOnUser: boolean; failed: number; premium: boolean;
  bytesPerSec: number | null; secondsLeft: number | null;
}
export interface QueueSnapshot { items: QueueItem[]; summary: QueueSummary }
export interface InstallRow {
  id: number; modId: number; fileId: number; name: string; version: string; modType: ModType; frameworks: string[]; staging: string;
  installedAt: number; updatedAt: string | null; deployedFiles: number; enabled: boolean; loadOrder: number;
}
export interface Conflict { target: string; ownerInstall: number; ownerName: string }
export type DeployOutcome =
  | { outcome: "deployed"; linked: number; copied: number; backedUp: string[]; tookFrom: Conflict[] }
  | { outcome: "conflicts"; conflicts: Conflict[] }
  | { outcome: "needsCopy"; reason: string };
export interface DeployOptions { allowCopy: boolean; takeConflicts: boolean }
export interface UndeployReport { removed: number; restored: string[]; leftAlone: string[] }
export interface DownloadRow { modId: number; fileId: number; name: string; version: string; path: string; size: number; md5: string; installed: boolean }
export interface PathsView {
  gameDir: string | null; libraryDir: string | null; defaultLibrary: string | null; allowCopy: boolean;
  gameLooksRight: boolean; sameVolume: boolean | null; inProgramFiles: boolean;
}
export interface HandlerStatus { supported: boolean; ours: boolean; owner: string | null; command: string | null }
export interface InstallPlan { files: Array<{ source: string; target: string; kind: string }>; skipped: string[]; unknown: string[]; modType: ModType | null; warnings: string[] }
export type InstallOutcome =
  | { status: "installed"; installId: number; modType: ModType; files: number; needsRedmodDeploy: boolean; deploy: DeployOutcome; replaced: number[]; warnings: string[] }
  | { status: "needsReview"; plan: InstallPlan }
  | { status: "staged"; installId: number; deploy: DeployOutcome };
export interface InstallRequest { modId: number; fileId: number; name: string; version: string; archive: string; acceptUnknown?: boolean; replaceOlder?: boolean; options?: DeployOptions }

export const library = {
  list: () => call<InstallRow[]>("library_list"),
  uninstall: (installId: number) => call<UndeployReport>("library_uninstall", { installId }),
  deploy: (installId: number, options: DeployOptions) => call<DeployOutcome>("library_deploy", { installId, options }),
  installFile: (request: InstallRequest) => call<InstallOutcome>("library_install_file", { request }),
  downloads: () => call<DownloadRow[]>("library_downloads"),
  onChanged: (cb: () => void) => listen<null>("library-changed", () => cb()),
};
export const queue = {
  get: () => call<QueueSnapshot>("queue_get"),
  plan: (modId: number, fileId: number) => call<QueueItem[]>("queue_plan", { modId, fileId }),
  add: (modId: number, fileId: number, withDependencies = true) => call<QueueSnapshot>("queue_add", { modId, fileId, withDependencies }),
  remove: (id: number) => call<QueueSnapshot>("queue_remove", { id }),
  retry: (id: number) => call<QueueSnapshot>("queue_retry", { id }),
  clearFinished: () => call<QueueSnapshot>("queue_clear_finished"),
  showPage: (id: number) => call<void>("queue_show_page", { id }),
  onChange: (cb: (s: QueueSnapshot) => void) => listen<QueueSnapshot>("queue", cb),
};
export const paths = {
  get: () => call<PathsView>("paths_get"),
  set: (p: { gameDir?: string; libraryDir?: string; allowCopy?: boolean }) => call<PathsView>("paths_set", p),
};
export const handler = {
  status: () => call<HandlerStatus>("nxm_handler_status"),
  register: () => call<HandlerStatus>("nxm_handler_register"),
};
export const nexusPages = {
  mod: (modId: number, fileId?: number) => call<void>("open_nexus_page", { modId, fileId }),
  url: (url: string) => call<void>("open_nexus_url", { url }),
};
export const onToast = (cb: (msg: string) => void) => listen<string>("toast", cb);

/** A folder picker, through the dialog plugin. Outside Tauri it asks with a prompt. */
export async function pickFolder(title: string): Promise<string | null> {
  if (!("__TAURI_INTERNALS__" in window)) return window.prompt(title) || null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: true, title });
  return typeof r === "string" ? r : null;
}
export async function pickArchive(): Promise<string | null> {
  if (!("__TAURI_INTERNALS__" in window)) return window.prompt("Archive path") || null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ title: "Install a mod archive", filters: [{ name: "Mod archives", extensions: ["zip", "7z", "rar"] }] });
  return typeof r === "string" ? r : null;
}
