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
export type SortKey = "downloads" | "endorsements" | "updated" | "created" | "relevance" | "size" | "name" | "random";

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
  seed?: number | null;
  offset: number;
  count: number;
}
export interface LocalFilter { showHidden: boolean; downloaded: boolean | null; skipCounts?: boolean }

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
  source: "pocket" | "vortex" | "found"; category: string | null; group: string; customGroup: boolean; picture?: string | null; author?: string | null;
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
  | { status: "staged"; installId: number; deploy: DeployOutcome }
  | { status: "needsChoices"; fomod: Fomod; defaults: Choice[] };
export interface InstallRequest { modId: number; fileId: number; name: string; version: string; archive: string; acceptUnknown?: boolean; replaceOlder?: boolean; options?: DeployOptions; fomodChoices?: Choice[] }
export type Choice = [number, number, number];
export interface FomodPlugin { name: string; description: string; image: string | null; kind: string }
export interface Fomod { name: string; root: string; steps: Array<{ name: string; groups: Array<{ name: string; kind: string; plugins: FomodPlugin[] }> }> }
export interface ChoicesNeeded { request: InstallRequest; fomod: Fomod; defaults: Choice[] }
export const onChoicesNeeded = (cb: (c: ChoicesNeeded) => void) => listen<ChoicesNeeded>("install-choices", cb);

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

// ── game, health, profiles ──

export interface GameInstall { store: string; path: string; exists: boolean }
export interface FrameworkState { framework: string; key: FrameworkKey; name: string; installed: boolean; version: string | null; modified: number | null; olderThanGame: boolean }
export interface GameInfo {
  path: string; exeFound: boolean; version: { file: string; product: string; productString: string | null } | null;
  detectedPatch: string | null; exeModified: number | null; redmodAvailable: boolean; frameworks: FrameworkState[]; writable: boolean;
}
export type Fix =
  | { kind: "installMod"; modId: number; name: string } | { kind: "updateMod"; modId: number; name: string }
  | { kind: "runRedmodDeploy" } | { kind: "redeploy"; installId: number } | { kind: "openLibrary" } | { kind: "openSettings" };
export interface Issue { id: string; severity: "problem" | "warning" | "info"; area: "game" | "frameworks" | "deploy" | "redmod" | "conflicts"; title: string; detail: string; fix: Fix | null }
export interface FrameworkRow extends FrameworkState { latest: string | null; updateAvailable: boolean; neededBy: string[] }
export interface HealthReport { game: GameInfo | null; patch: string | null; patchSource: "detected" | "settings" | "assumed"; frameworks: FrameworkRow[]; issues: Issue[]; problems: number; warnings: number }
export interface RedmodResult { ok: boolean; mods: string[]; problems: string[]; log: string }
export interface RedmodEntry { installId: number; name: string; folders: string[]; enabled: boolean; loadOrder: number }
export interface Profile { id: number; name: string; active: boolean; enabled: number }
export interface SwitchReport { removed: number[]; deployed: number[]; notDeployed: Array<[number, string]> }
export interface Update { modId: number; name: string; installed: string; latest: string; updatedAt: string }

export const game = {
  detect: () => call<GameInstall[]>("game_detect"),
  inspect: () => call<GameInfo | null>("game_inspect"),
  health: (refresh = false) => call<HealthReport>("health_check", { refresh }),
  redmodDeploy: () => call<RedmodResult>("redmod_deploy"),
  redmodOrder: () => call<RedmodEntry[]>("redmod_order"),
};
export const profiles = {
  list: () => call<Profile[]>("profiles_list"),
  create: (name: string, copyFrom?: number) => call<number>("profile_create", { name, copyFrom }),
  rename: (id: number, name: string) => call<void>("profile_rename", { id, name }),
  remove: (id: number) => call<void>("profile_delete", { id }),
  activate: (id: number) => call<SwitchReport>("profile_activate", { id }),
  setEnabled: (installId: number, on: boolean) => call<DeployOutcome | null>("mod_set_enabled", { installId, on }),
  setOrder: (installIds: number[]) => call<void>("load_order_set", { installIds }),
};
export const updates = {
  check: () => call<Update[]>("updates_check"),
  last: () => call<Update[]>("updates_last"),
  onChange: (cb: (u: Update[]) => void) => listen<Update[]>("updates", cb),
};
export const queueMod = (modId: number) => call<QueueSnapshot>("queue_add_mod", { modId });

// ── collections ──

export interface CollectionCard {
  id: number; slug: string; name: string; summary: string; endorsements: number; totalDownloads: number; adultContent: boolean | null;
  user: { name: string }; tileImage: { url: string; thumbnailUrl: string | null } | null; category: { name: string } | null;
  latestPublishedRevision: { revisionNumber: number; modCount: number; totalSize: number; adultContent: boolean; gameVersions: Array<{ reference: string }> } | null;
}
export interface CollectionsPage { totalCount: number; facetsData: { gameVersion?: Record<string, number> } | null; nodes: CollectionCard[] }
export interface CollectionQuery { text?: string | null; gameVersion?: string | null; sort?: string | null; includeAdult: boolean; offset: number }
export interface CollectionEntry {
  modId: number; fileId: number; name: string; fileName: string; version: string; size: number; uri: string; optional: boolean; md5: string | null;
  status: "installed" | "other" | "missing" | "unavailable"; replacement: [number, string] | null;
}
export interface CollectionCheck {
  name: string; collectionVersions: string[]; currentPatch: string | null; gameVersionOk: boolean | null; entries: CollectionEntry[];
  missing: number; installed: number; unavailable: number; needsClick: number; downloadBytes: number;
}
export interface LocalCollection {
  format: string; version: number; name: string; description: string; game: string; gameVersion: string | null; createdAt: string;
  mods: Array<{ modId: number; fileId: number; name: string; fileName: string; version: string; md5: string | null; size: number; optional: boolean; requires: number[]; uri: string }>;
  loadOrder: number[];
}
export const collections = {
  browse: (query: CollectionQuery) => call<CollectionsPage>("collections_browse", { query }),
  check: (slug: string, revision?: number) => call<CollectionCheck>("collection_check", { slug, revision }),
  install: (check: CollectionCheck, includeOptional: boolean) => call<QueueSnapshot>("collection_install", { check, includeOptional }),
  createLocal: (name: string, description: string) => call<LocalCollection>("local_collection_create", { name, description }),
  exportLocal: (collection: LocalCollection, path: string) => call<void>("local_collection_export", { collection, path }),
  importLocal: (path: string) => call<[LocalCollection, CollectionCheck]>("local_collection_import", { path }),
};
export async function pickSavePath(defaultName: string): Promise<string | null> {
  if (!("__TAURI_INTERNALS__" in window)) return window.prompt("Save as", defaultName) || null;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return (await save({ title: "Export collection", defaultPath: defaultName, filters: [{ name: "Pocket Mods collection", extensions: ["pmcollection.json", "json"] }] })) ?? null;
}
export async function pickCollectionFile(): Promise<string | null> {
  if (!("__TAURI_INTERNALS__" in window)) return window.prompt("Collection file") || null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ title: "Import a collection", filters: [{ name: "Pocket Mods collection", extensions: ["json"] }] });
  return typeof r === "string" ? r : null;
}

// ── app updates and mod.io (experiment) ──

export interface AppUpdate { current: string; latest: string | null; newer: boolean; notes: string | null; url: string | null; message: string }
export interface ModioMod {
  id: number; name: string; summary: string; profile_url: string; date_updated: number; logo: { thumb_320x180?: string | null };
  submitted_by: { username: string }; stats: { downloads_total: number; subscribers_total: number }; modfile: { filesize: number; version?: string | null } | null;
}
export interface ModioBrowse { game: { id: number; name: string } | null; page: { data: ModioMod[]; result_total: number; result_offset: number } | null }
export const extras = {
  appUpdate: () => call<AppUpdate>("app_update_check"),
  modioHasKey: () => call<boolean>("modio_has_key"),
  modioSetKey: (key: string) => call<{ id: number; name: string } | null>("modio_set_key", { key }),
  modioClearKey: () => call<void>("modio_clear_key"),
  modioBrowse: (text: string, offset: number) => call<ModioBrowse>("modio_browse", { text, offset }),
};

// ── media ──

export interface MediaItem {
  __typename: "Image" | "Video" | "SupporterImage"; id: string; name?: string; title?: string | null; url?: string; link?: string;
  thumbnailUrl: string; rating: number; views: number; createdAt: string; owner: { name: string };
}
export interface MediaPage { totalCount: number; nodes: MediaItem[] }
export const media = { browse: (kind: "image" | "video", sort: "rating" | "views" | "createdAt", offset: number) => call<MediaPage>("media_browse", { kind, sort, offset }) };

/** Images and videos a mod's description shows: [img] tags, <img> tags, and YouTube links. */
export function descriptionMedia(src: string, picture?: string | null): Array<{ kind: "image" | "video"; url: string; thumb: string }> {
  const out: Array<{ kind: "image" | "video"; url: string; thumb: string }> = [];
  const seen = new Set<string>();
  const add = (kind: "image" | "video", url: string, thumb = url) => { if (!seen.has(url)) { seen.add(url); out.push({ kind, url, thumb }); } };
  if (picture) add("image", picture);
  for (const m of src.matchAll(/\[img[^\]]*\]\s*(https?:\/\/[^\s[\]]+?)\s*\[\/img\]/gi)) add("image", m[1]);
  for (const m of src.matchAll(/<img[^>]+src=["'](https?:\/\/[^"']+)["']/gi)) add("image", m[1]);
  const yt = (id: string) => add("video", `https://www.youtube-nocookie.com/embed/${id}`, `https://img.youtube.com/vi/${id}/mqdefault.jpg`);
  for (const m of src.matchAll(/\[youtube\]\s*([A-Za-z0-9_-]{11})\s*\[\/youtube\]/gi)) yt(m[1]);
  for (const m of src.matchAll(/(?:youtube\.com\/watch\?v=|youtu\.be\/|youtube\.com\/embed\/)([A-Za-z0-9_-]{11})/gi)) yt(m[1]);
  return out.slice(0, 40);
}

// ── existing mods, groups, bulk ──

export interface FoundMod { name: string; kind: ModType; files: string[]; modId: number | null }
export interface ExistingScan { vortex: { staging: string | null; mods: number; deployed: number; alreadyTracked: number; sample: string[] } | null; found: FoundMod[] }
export interface TrackResult { vortex: { tracked: number; matchedOnNexus: number; already: number }; found: number }
export const existing = {
  scan: () => call<ExistingScan>("existing_scan"),
  track: (vortexMods: boolean, foundMods: boolean) => call<TrackResult>("existing_track", { vortexMods, foundMods }),
};
export const groups = {
  list: () => call<{ presets: string[]; custom: string[] }>("groups_list"),
  set: (installIds: number[], group: string | null) => call<void>("groups_set", { installIds, group }),
  create: (name: string) => call<void>("group_create", { name }),
  rename: (from: string, to: string) => call<void>("group_rename", { from, to }),
  remove: (name: string) => call<void>("group_delete", { name }),
};
export const bulk = (installIds: number[], action: "enable" | "disable" | "uninstall") => call<{ done: number; failed: Array<[number, string]> }>("library_bulk", { installIds, action });

export const launchGame = () => call<{ started: boolean; modded?: boolean; redmod?: RedmodResult | null }>("game_launch");

export const downloadsUsage = () => call<{ files: number; bytes: number; installedFiles: number; installedBytes: number }>("downloads_usage");
export const downloadsCleanup = () => call<number>("downloads_cleanup");
export const testHandler = () => call<void>("nxm_handler_test");

// ── for mod authors ──

export interface CreatorCheck { plan: InstallPlan; problems: string[]; tips: string[]; files: number }
export interface PublishRequest { file: string; fileId: number; modId?: number | null; version: string; displayName?: string | null; description?: string | null; category: "main" | "optional" | "miscellaneous"; changelog?: string | null; archiveExisting: boolean; updateModVersion: boolean }
export const creator = {
  check: (path: string, name: string) => call<CreatorCheck>("creator_check", { path, name }),
  pack: (path: string, name: string, version: string) => call<string>("creator_package", { path, name, version }),
  publish: (request: PublishRequest) => call<number>("creator_publish", { request }),
  onProgress: (cb: (p: [number, number]) => void) => listen<[number, number]>("publish-progress", cb),
};

// ── text files the user picks ──

export const readTextFile = (path: string) => call<string>("text_file_read", { path });
export const saveTextFile = (path: string, text: string) => call<void>("text_file_save", { path, text });
/** Ask for a file to open. Outside Tauri, a browser file picker reads it and the content comes back as `text`. */
export async function pickTextFile(title: string, extensions: string[]): Promise<{ path: string | null; text: string } | null> {
  if (!("__TAURI_INTERNALS__" in window)) {
    return new Promise((resolve) => {
      const input = document.createElement("input");
      input.type = "file";
      input.accept = extensions.map((e) => `.${e}`).join(",");
      input.onchange = () => { const f = input.files?.[0]; if (!f) return resolve(null); f.text().then((text) => resolve({ path: null, text })); };
      input.click();
    });
  }
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ title, filters: [{ name: title, extensions }] });
  if (typeof r !== "string") return null;
  return { path: r, text: await readTextFile(r) };
}
/** Save text to a file the user picks. Outside Tauri, the browser downloads it. */
export async function saveTextAs(defaultName: string, text: string, extensions: string[]): Promise<boolean> {
  if (!("__TAURI_INTERNALS__" in window)) {
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([text], { type: "application/json" }));
    a.download = defaultName;
    a.click();
    return true;
  }
  const { save } = await import("@tauri-apps/plugin-dialog");
  const p = await save({ defaultPath: defaultName, filters: [{ name: defaultName, extensions }] });
  if (!p) return false;
  await saveTextFile(p, text);
  return true;
}

// ── add-ons: installed themes and extensions, and the marketplace ──

export type AddonKind = "theme" | "extension";
export const addons = {
  /** Each installed add-on's JSON text. */
  list: (kind: AddonKind) => call<string[]>("addon_list", { kind }),
  save: (kind: AddonKind, id: string, json: string) => call<void>("addon_save", { kind, id, json }),
  remove: (kind: AddonKind, id: string) => call<void>("addon_remove", { kind, id }),
};
/** A JSON file from the marketplace folder (`index.json`, `themes/x.json` …), cached for an hour. */
export const marketGet = (path: string, fresh = false) => call<string>("market_get", { path, fresh });

// ── extensions: state, tools, launch options, links; pages in the default browser ──

export interface ExtState {
  enabled: boolean; approvedRelease: string | null; approved: string[]; toolPaths: Record<string, string>; fromMarket: boolean; installedAt: number | null;
}
export const extensionsApi = {
  states: () => call<Record<string, ExtState>>("ext_states"),
  setState: (id: string, state: ExtState | null) => call<void>("ext_state_set", { id, state }),
  startTool: (extId: string, toolId: string) => call<void>("ext_tool_start", { extId, toolId }),
  launch: (extId: string, optionId: string) => call<{ started: boolean; modded?: boolean; redmod?: RedmodResult | null; args?: string[] }>("ext_launch", { extId, optionId }),
  openLink: (extId: string, linkId: string) => call<void>("ext_open_link", { extId, linkId }),
};
/** Open a page in the default browser. Only this project's GitHub pages and Nexus Mods pages. */
export const openExternal = (url: string) => call<void>("open_external", { url });
/** A marketplace file and where it came from: the network, the hour's cache, or an old copy when offline. */
export const marketFetch = (path: string, fresh = false) => call<{ text: string; source: "network" | "cache" | "offline" }>("market_fetch", { path, fresh });
/** Ask for a program (an extension's tool). Outside Tauri it asks with a prompt. */
export async function pickProgram(title: string): Promise<string | null> {
  if (!("__TAURI_INTERNALS__" in window)) return window.prompt(title) || null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ title, filters: [{ name: "Programs", extensions: ["exe"] }] });
  return typeof r === "string" ? r : null;
}
