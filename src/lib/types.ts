// Shapes from pocket-core, as the Tauri commands send them (serde camelCase).

export type Framework = "cet" | "red4ext" | "redscript" | "archiveXl" | "tweakXl" | "codeware" | "cybercmd";
export type ModKind = "redmod" | "archive" | "script" | "framework" | "other";
export type Compat = "updated" | "likely" | "risky" | "unknown";
export type SortKey = "relevance" | "downloads" | "endorsements" | "updated" | "created" | "size" | "name";
export type Level = "ok" | "low" | "throttled";
export type Status = "ok" | "warn" | "bad";
export type ItemState = "waiting" | "needsClick" | "resolving" | "downloading" | "installing" | "done" | "failed";

export interface ApiError {
  kind: string;
  message: string;
}

export interface Derived {
  frameworks: Framework[];
  kind: ModKind | null;
  sources: string[];
}

export interface ModCard {
  modId: number;
  name: string;
  summary: string;
  author: string;
  thumbnailUrl: string | null;
  pictureUrl: string | null;
  downloads: number;
  endorsements: number;
  updatedAt: number;
  createdAt: number;
  sizeBytes: number | null;
  category: string | null;
  adult: boolean;
  version: string | null;
  derived: Derived;
  compat: Compat;
  hidden: boolean;
  downloaded: boolean;
  installed: boolean;
}

export interface FacetCount {
  value: string;
  count: number;
}

export interface Facets {
  categories: FacetCount[];
  tags: FacetCount[];
  frameworks: FacetCount[];
  kinds: FacetCount[];
  compatible: number;
  downloaded: number;
  notDownloaded: number;
  hidden: number;
}

export interface ModPage {
  mods: ModCard[];
  total: number;
  offset: number;
  facets: Facets;
  cached: boolean;
  filteredOut: number;
}

export interface BrowseQuery {
  text?: string | null;
  categories: string[];
  tags: string[];
  author?: string | null;
  sort: SortKey;
  ascending: boolean;
  updatedAfter?: number | null;
  updatedBefore?: number | null;
  createdAfter?: number | null;
  createdBefore?: number | null;
  minDownloads?: number | null;
  maxDownloads?: number | null;
  minEndorsements?: number | null;
  maxEndorsements?: number | null;
  minSizeKb?: number | null;
  maxSizeKb?: number | null;
  includeAdult: boolean;
  offset: number;
  count: number;
  frameworks: Framework[];
  kinds: ModKind[];
  compatibleOnly: boolean;
  showHidden: boolean;
  downloaded?: boolean | null;
}

export interface ModFile {
  fileId: number;
  name: string;
  version: string | null;
  category: string;
  sizeBytes: number;
  fileName: string;
  uploadedAt: number;
  isPrimary: boolean;
  description: string | null;
  contentPreviewUrl: string | null;
}

export interface Requirement {
  modId: number | null;
  name: string;
  url: string | null;
  notes: string | null;
  external: boolean;
}

export interface ModDetail {
  card: ModCard;
  description: string;
  files: ModFile[];
  requirements: Requirement[];
  recommendedFileId: number | null;
  endorseStatus: string | null;
  tracked: boolean | null;
  previewFiles: string[];
  manualOnly: boolean;
}

export interface Category {
  id: number;
  name: string;
  parent: number | null;
}

export interface Account {
  userId: number;
  name: string;
  isPremium: boolean;
  isSupporter: boolean;
  profileUrl: string | null;
}

export interface RateStatus {
  dailyLimit: number | null;
  dailyRemaining: number | null;
  dailyReset: string | null;
  hourlyLimit: number | null;
  hourlyRemaining: number | null;
  hourlyReset: string | null;
  throttledUntil: number | null;
  updatedAt: number;
  level: Level;
  remaining: number | null;
}

export interface QueueSummary {
  left: number;
  needsClick: number;
  bytesLeft: number;
  etaSecs: number | null;
  waitingOnUser: boolean;
  running: boolean;
  done: number;
  failed: number;
  active: number | null;
  percent: number;
}

export interface QueueItem {
  id: number;
  modId: number;
  fileId: number;
  modName: string;
  fileName: string;
  version: string | null;
  sizeBytes: number;
  received: number;
  state: ItemState;
  error: string | null;
  manualOnly: boolean;
  origin: string;
  addedAt: number;
}

export interface QueueView {
  items: QueueItem[];
  summary: QueueSummary;
  premium: boolean;
  paused: boolean;
}

export type Launcher = "steam" | "gog" | "epic" | "manual";

export interface GameInstall {
  root: string;
  launcher: Launcher;
  version: string | null;
  buildTime: number | null;
  underProgramFiles: boolean;
  redmodTool: boolean;
}

export interface AppStatus {
  hasKey: boolean;
  account: Account | null;
  game: GameInstall | null;
  storeRoot: string | null;
  rate: RateStatus;
  queue: QueueSummary;
  updates: number;
  profile: string;
  version: string;
}

export interface Experiments {
  publishCollections: boolean;
  modio: boolean;
}

export interface Settings {
  gameRoot: string | null;
  storeRoot: string | null;
  allowCopy: boolean;
  laterWins: boolean;
  theme: "system" | "cream" | "night";
  motion: boolean;
  includeAdult: boolean;
  redmodAutoDeploy: boolean;
  checkAppUpdates: boolean;
  modioGameId: number | null;
  experiments: Experiments;
}

export interface UpdateInfo {
  fileId: number;
  version: string | null;
  uploadedAt: number;
  fileName: string;
  sizeBytes: number;
}

export interface InstalledMod {
  key: string;
  modId: number | null;
  fileId: number | null;
  name: string;
  fileName: string;
  version: string | null;
  author: string | null;
  kind: ModKind;
  frameworks: Framework[];
  requirements: number[];
  md5: string | null;
  size: number;
  installedAt: number;
  uploadedAt: number | null;
  archivePath: string | null;
  redmods: string[];
  thumbnail: string | null;
  unplaced: string[];
  updateAvailable: UpdateInfo | null;
  endorsed: boolean | null;
}

export interface LibraryRow extends InstalledMod {
  enabled: boolean;
  deployed: boolean;
  drift: number;
  files: number;
}

export interface FileConflict {
  rel: string;
  otherMod: string;
  won: boolean;
}

export interface InstallReport {
  key: string;
  name: string;
  kind: ModKind;
  placed: number;
  skippedDocs: number;
  unplaced: string[];
  conflicts: FileConflict[];
  copied: number;
  replaced: string | null;
  redmods: string[];
}

export interface DeployOutcome {
  ok: boolean;
  problems: string[];
  logTail: string[];
  at: number;
}

export interface RedmodRow {
  folder: string;
  name: string;
  version: string | null;
  owner: string | null;
}

export interface LoadOrderView {
  rows: RedmodRow[];
  tool: boolean;
  dirty: boolean;
  last: DeployOutcome | null;
}

export type FixAction =
  | { type: "setGameFolder" }
  | { type: "installFramework"; framework: Framework; modId: number }
  | { type: "updateFramework"; framework: Framework; modId: number }
  | { type: "runRedmodDeploy" }
  | { type: "installRedmodDlc" }
  | { type: "redeploy"; key: string }
  | { type: "checkUpdates" };

export interface Fix {
  action: FixAction;
  label: string;
}

export interface HealthItem {
  title: string;
  detail: string;
  status: Status;
  fix: Fix | null;
}

export interface HealthModule {
  id: string;
  title: string;
  status: Status;
  items: HealthItem[];
}

export interface HealthReport {
  modules: HealthModule[];
  worst: Status;
  primaryFix: Fix | null;
  problems: number;
}

export interface CollectionCard {
  slug: string;
  name: string;
  summary: string;
  author: string;
  thumbnailUrl: string | null;
  endorsements: number;
  downloads: number;
  revision: number | null;
  modCount: number;
  sizeBytes: number;
  adult: boolean;
  gameVersions: string[];
  category: string | null;
}

export interface CollectionMod {
  name: string;
  version: string;
  optional: boolean;
  domainName: string;
  source: {
    type: string;
    url?: string | null;
    modId?: number | null;
    fileId?: number | null;
    md5?: string | null;
    fileSize?: number | null;
    logicalFilename?: string | null;
    updatePolicy?: string | null;
    instructions?: string | null;
  };
  author?: string | null;
}

export interface Collection {
  info: {
    author: string;
    authorUrl: string;
    name: string;
    description: string;
    domainName: string;
    gameVersions: string[];
    installInstructions?: string | null;
  };
  mods: CollectionMod[];
  modRules: { source: unknown; type: string; reference: unknown }[];
  pocketMods?: unknown;
}

export interface MissingMod {
  name: string;
  modId: number | null;
  fileId: number | null;
  optional: boolean;
  external: boolean;
  url: string | null;
  sizeBytes: number;
}

export interface PreCheck {
  name: string;
  total: number;
  optional: number;
  installed: number;
  missing: MissingMod[];
  manualClicks: number;
  external: number;
  bytesToDownload: number;
  gameVersionOk: boolean | null;
  gameVersions: string[];
}

export interface HandlerStatus {
  supported: boolean;
  ours: boolean;
  command: string | null;
  holder: string | null;
  previous: string | null;
}

export interface Release {
  version: string;
  url: string;
  notes: string;
  newer: boolean;
}

export interface ModioMod {
  id: number;
  name: string;
  summary: string;
  author: string;
  logoUrl: string | null;
  downloads: number;
  updatedAt: number;
  sizeBytes: number;
  profileUrl: string | null;
}

export const FRAMEWORK_NAMES: Record<Framework, string> = {
  cet: "Cyber Engine Tweaks",
  red4ext: "RED4ext",
  redscript: "redscript",
  archiveXl: "ArchiveXL",
  tweakXl: "TweakXL",
  codeware: "Codeware",
  cybercmd: "cybercmd",
};

export const FRAMEWORK_SHORT: Record<Framework, string> = {
  cet: "CET",
  red4ext: "RED4ext",
  redscript: "redscript",
  archiveXl: "ArchiveXL",
  tweakXl: "TweakXL",
  codeware: "Codeware",
  cybercmd: "cybercmd",
};

export const KIND_NAMES: Record<ModKind, string> = {
  redmod: "REDmod",
  archive: "Archive",
  script: "Script",
  framework: "Framework",
  other: "Other",
};

/** Tag colours (--k1..--k6) per mod type. */
export const KIND_COLOR: Record<ModKind, string> = {
  redmod: "var(--k5)",
  archive: "var(--k1)",
  script: "var(--k2)",
  framework: "var(--k4)",
  other: "var(--text-3)",
};

export const ALL_FRAMEWORKS: Framework[] = ["cet", "red4ext", "redscript", "archiveXl", "tweakXl", "codeware", "cybercmd"];
export const ALL_KINDS: ModKind[] = ["archive", "script", "redmod", "framework", "other"];

export function emptyQuery(): BrowseQuery {
  return {
    text: "",
    categories: [],
    tags: [],
    sort: "relevance",
    ascending: false,
    includeAdult: false,
    offset: 0,
    count: 20,
    frameworks: [],
    kinds: [],
    compatibleOnly: false,
    showHidden: false,
    downloaded: null,
  };
}
