// An in-memory stand-in for pocket-core, used outside Tauri (vite dev in a browser, tests, screenshots).
// Its mods are invented sample data, except the framework names. It mimics a free account by default: queued
// mods wait for a click, and "opening" a page simulates that click after a moment.

import { emitLocal } from "./api";
import type * as T from "./types";

const now = () => Math.floor(Date.now() / 1000);
const DAY = 86400;
const BUILD = now() - 40 * DAY;

type Seed = [number, string, string, string, T.ModKind, T.Framework[], number, number, number, number, string];
// id, name, author, summary, kind, frameworks, downloads, endorsements, size MB, days since update, category
const SEEDS: Seed[] = [
  [107, "Cyber Engine Tweaks", "yamashi", "Scripting framework, console and fixes.", "framework", [], 9_800_000, 210_000, 22, 12, "Modders Resources"],
  [2380, "RED4ext", "WopsS", "A script extender for REDengine 4.", "framework", [], 7_100_000, 98_000, 3, 18, "Modders Resources"],
  [1511, "redscript", "jekky", "Compiler and tooling for game scripts.", "framework", [], 7_400_000, 101_000, 4, 9, "Modders Resources"],
  [4198, "ArchiveXL", "psiberx", "Load custom resources without touching game files.", "framework", ["red4ext"], 6_300_000, 77_000, 2, 15, "Modders Resources"],
  [4197, "TweakXL", "psiberx", "Load custom tweaks and records.", "framework", ["red4ext"], 6_000_000, 72_000, 2, 21, "Modders Resources"],
  [7780, "Codeware", "psiberx", "Library and framework for redscript and CET mods.", "framework", ["red4ext", "redscript"], 4_800_000, 51_000, 2, 30, "Modders Resources"],
  [90001, "Rainy Night City", "lumen_ops", "Wetter streets, denser rain and puddles that hold the neon.", "archive", [], 412_000, 9_800, 340, 120, "Visuals and Graphics"],
  [90002, "Better Vehicle Handling", "torque", "Reworked grip and weight for every car. Requires TweakXL.", "script", ["tweakXl", "red4ext"], 1_200_000, 31_000, 1, 8, "Vehicles"],
  [90003, "Street Vendor Expansion", "kiosk", "More vendors with stock that changes by district. Needs CET and Codeware.", "script", ["cet", "codeware", "redscript", "red4ext"], 280_000, 6_100, 6, 200, "Gameplay"],
  [90004, "Techwear Jacket Pack", "stitch", "Twelve jackets, ArchiveXL, every body type.", "archive", ["archiveXl", "red4ext"], 520_000, 14_000, 180, 50, "Clothing"],
  [90005, "Immersive Radio Stations", "airwave", "Six new stations as a REDmod.", "redmod", [], 345_000, 8_200, 950, 400, "Audio"],
  [90006, "No Intro Videos", "skip", "Skips the logos at launch.", "archive", [], 2_100_000, 40_000, 0.2, 700, "Miscellaneous"],
  [90007, "Crowd Density Tweaks", "people", "More pedestrians without the stutter. Uses redscript.", "script", ["redscript"], 800_000, 19_000, 0.1, 90, "Gameplay"],
  [90008, "HUD Cleaner", "minimal", "Hide parts of the HUD from a CET menu.", "script", ["cet"], 650_000, 17_000, 0.5, 5, "User Interface"],
  [90009, "Photo Mode Unlocker", "shutter", "Free camera range and more poses.", "script", ["cet", "redscript"], 900_000, 23_000, 1, 300, "Photo Mode"],
  [90010, "Neon Apartment", "decor", "A new apartment in Japantown, as a REDmod.", "redmod", ["archiveXl", "red4ext"], 130_000, 3_400, 420, 33, "Locations"],
];

const cards: T.ModCard[] = SEEDS.map(([modId, name, author, summary, kind, fw, dl, en, mb, days, cat]) => ({
  modId,
  name,
  summary,
  author,
  thumbnailUrl: null,
  pictureUrl: null,
  downloads: dl,
  endorsements: en,
  updatedAt: now() - days * DAY,
  createdAt: now() - (days + 400) * DAY,
  sizeBytes: Math.round(mb * 1024 * 1024),
  category: cat,
  adult: false,
  version: `${1 + (modId % 3)}.${modId % 10}`,
  derived: { frameworks: fw, kind, sources: ["text"] },
  compat: "unknown",
  hidden: false,
  downloaded: false,
  installed: false,
}));

const state = {
  hasKey: false,
  account: null as T.Account | null,
  settings: {
    gameRoot: "D:\\SteamLibrary\\steamapps\\common\\Cyberpunk 2077",
    storeRoot: null,
    allowCopy: false,
    laterWins: true,
    theme: "system",
    motion: true,
    includeAdult: false,
    redmodAutoDeploy: true,
    checkAppUpdates: true,
    modioGameId: null,
    experiments: { publishCollections: false, modio: false },
  } as T.Settings,
  hidden: new Set<number>(),
  downloaded: new Set<number>([90006]),
  library: [] as T.LibraryRow[],
  queue: { items: [] as T.QueueItem[], premium: false, paused: false, nextId: 1 },
  profiles: ["Default"],
  active: "Default",
  order: [] as string[],
  redmodDirty: false,
  last: null as T.DeployOutcome | null,
  rate: { daily: 19_842, hourly: 500 },
  handler: { ours: false, holder: "Vortex" as string | null },
  saved: [] as { name: string; collection: T.Collection }[],
};

function installedMod(c: T.ModCard, fileId: number): T.LibraryRow {
  return {
    key: `${c.modId}-${fileId}`,
    modId: c.modId,
    fileId,
    name: c.name,
    fileName: `${c.name.replace(/\s+/g, "_")}.zip`,
    version: c.version,
    author: c.author,
    kind: c.derived.kind ?? "other",
    frameworks: c.derived.frameworks,
    requirements: [],
    md5: "0f3c" + String(c.modId).padStart(28, "0"),
    size: c.sizeBytes ?? 0,
    installedAt: now() - 3 * DAY,
    uploadedAt: c.updatedAt,
    archivePath: null,
    redmods: c.derived.kind === "redmod" ? [c.name.replace(/\s+/g, "")] : [],
    thumbnail: null,
    unplaced: [],
    updateAvailable: null,
    endorsed: null,
    enabled: true,
    deployed: true,
    drift: 0,
    files: 4,
  };
}

// a few installed to start with
for (const id of [107, 2380, 1511, 90006, 90005, 90008]) {
  const c = cards.find((x) => x.modId === id)!;
  state.library.push(installedMod(c, id * 10));
}
state.library.find((m) => m.modId === 90008)!.updateAvailable = { fileId: 900081, version: "2.1", uploadedAt: now() - DAY, fileName: "HUD_Cleaner_2.1.zip", sizeBytes: 500_000 };
state.order = state.library.flatMap((m) => m.redmods);

function rate(): T.RateStatus {
  const hourlyLow = state.rate.daily === 0;
  const level: T.Level = state.rate.daily === 0 && state.rate.hourly === 0 ? "throttled" : hourlyLow && state.rate.hourly < 60 ? "low" : state.rate.daily < 500 && !hourlyLow ? "low" : "ok";
  return {
    dailyLimit: 20000,
    dailyRemaining: state.rate.daily,
    dailyReset: "00:00 GMT",
    hourlyLimit: 500,
    hourlyRemaining: state.rate.hourly,
    hourlyReset: null,
    throttledUntil: null,
    updatedAt: now(),
    level,
    remaining: state.rate.daily > 0 ? state.rate.daily : state.rate.hourly,
  };
}

function spend() {
  if (state.rate.daily > 0) state.rate.daily -= 1;
  else state.rate.hourly = Math.max(0, state.rate.hourly - 1);
}

function compat(c: T.ModCard): T.Compat {
  if (c.updatedAt >= BUILD) return "updated";
  return c.derived.kind === "archive" && c.derived.frameworks.length === 0 ? "likely" : "risky";
}

function decorate(c: T.ModCard): T.ModCard {
  return {
    ...c,
    compat: compat(c),
    hidden: state.hidden.has(c.modId),
    downloaded: state.downloaded.has(c.modId),
    installed: state.library.some((m) => m.modId === c.modId),
  };
}

function summary(): T.QueueSummary {
  const items = state.queue.items;
  const active = items.find((i) => ["resolving", "downloading", "installing"].includes(i.state));
  const left = items.filter((i) => !["done", "failed"].includes(i.state));
  const bytesLeft = left.reduce((s, i) => s + Math.max(0, i.sizeBytes - i.received), 0);
  const total = items.filter((i) => i.state !== "failed").reduce((s, i) => s + Math.max(1, i.sizeBytes), 0);
  const got = items.filter((i) => i.state !== "failed").reduce((s, i) => s + (i.state === "done" ? Math.max(1, i.sizeBytes) : i.received), 0);
  return {
    left: left.length,
    needsClick: state.queue.premium ? left.filter((i) => i.manualOnly && !active).length : left.filter((i) => i.state === "waiting" || i.state === "needsClick").length,
    bytesLeft,
    etaSecs: active ? Math.ceil(bytesLeft / 6_000_000) : null,
    waitingOnUser: !active && items.some((i) => i.state === "needsClick"),
    running: !!active,
    done: items.filter((i) => i.state === "done").length,
    failed: items.filter((i) => i.state === "failed").length,
    active: active?.id ?? null,
    percent: total ? Math.round((got / total) * 100) : 0,
  };
}

function queueView(): T.QueueView {
  return { items: state.queue.items.map((i) => ({ ...i })), summary: summary(), premium: state.queue.premium, paused: state.queue.paused };
}

function emitQueue() {
  emitLocal("queue", queueView());
}

function pump() {
  const q = state.queue;
  if (q.paused || q.items.some((i) => ["resolving", "downloading", "installing"].includes(i.state))) return emitQueue();
  const next = q.items.find((i) => i.state === "waiting" || i.state === "needsClick");
  if (!next) return emitQueue();
  if (q.premium && !next.manualOnly) {
    run(next);
  } else {
    next.state = "needsClick";
  }
  emitQueue();
}

function run(item: T.QueueItem) {
  item.state = "resolving";
  spend();
  emitQueue();
  let got = 0;
  const step = Math.max(1, Math.ceil(item.sizeBytes / 8));
  const t = setInterval(() => {
    got = Math.min(item.sizeBytes, got + step);
    item.state = "downloading";
    item.received = got;
    emitQueue();
    if (got >= item.sizeBytes) {
      clearInterval(t);
      item.state = "installing";
      emitQueue();
      setTimeout(() => {
        item.state = "done";
        const c = cards.find((x) => x.modId === item.modId);
        if (c) {
          state.library = state.library.filter((m) => m.modId !== c.modId);
          state.library.push(installedMod(c, item.fileId));
          state.downloaded.add(c.modId);
          if (c.derived.kind === "redmod") {
            state.order.push(c.name.replace(/\s+/g, ""));
            state.redmodDirty = true;
          }
        }
        emitLocal("installed", { name: item.modName, placed: 4, conflicts: [], unplaced: [], kind: c?.derived.kind ?? "other" });
        pump();
      }, 400);
    }
  }, 300);
}

function enqueue(c: T.ModCard, fileId: number, origin: string): number {
  const ex = state.queue.items.find((i) => i.modId === c.modId && i.fileId === fileId && i.state !== "done");
  if (ex) return ex.id;
  const id = state.queue.nextId++;
  state.queue.items.push({
    id,
    modId: c.modId,
    fileId,
    modName: c.name,
    fileName: `${c.name.replace(/\s+/g, "_")}.zip`,
    version: c.version,
    sizeBytes: c.sizeBytes ?? 1_000_000,
    received: 0,
    state: "waiting",
    error: null,
    manualOnly: false,
    origin,
    addedAt: now(),
  });
  return id;
}

function detail(modId: number): T.ModDetail {
  const c = cards.find((x) => x.modId === modId);
  if (!c) throw { kind: "notFound", message: `Nexus has no mod ${modId}.` };
  const fileId = modId * 10;
  return {
    card: decorate(c),
    description: `[b]${c.name}[/b]\n${c.summary}\n\n[heading]Install[/heading]\n[list][*]Use Pocket Mods (one click)[*]Or extract into the game folder[/list]\nSee [url=https://www.nexusmods.com/cyberpunk2077/mods/${modId}]the mod page[/url] for notes.`,
    files: [
      { fileId, name: `${c.name} main`, version: c.version, category: "MAIN", sizeBytes: c.sizeBytes ?? 0, fileName: `${c.name}.zip`, uploadedAt: c.updatedAt, isPrimary: true, description: null, contentPreviewUrl: null },
      { fileId: fileId + 1, name: "Old version", version: "0.9", category: "OLD_VERSION", sizeBytes: (c.sizeBytes ?? 0) * 0.9, fileName: `${c.name}-old.zip`, uploadedAt: c.updatedAt - 90 * DAY, isPrimary: false, description: null, contentPreviewUrl: null },
    ],
    requirements: c.derived.frameworks.map((f) => {
      const fwCard = cards.find((x) => x.derived.kind === "framework" && x.name.toLowerCase().replace(/\s/g, "").includes(f.toLowerCase().replace("xl", "xl")));
      return { modId: fwCard?.modId ?? null, name: fwCard?.name ?? f, url: null, notes: null, external: false };
    }),
    recommendedFileId: fileId,
    endorseStatus: "Undecided",
    tracked: false,
    previewFiles: c.derived.kind === "archive" ? ["archive/pc/mod/a.archive"] : [],
    manualOnly: false,
  };
}

function browse(q: T.BrowseQuery): T.ModPage {
  spend();
  const text = (q.text ?? "").toLowerCase().trim();
  let list = cards.filter((c) => !text || c.name.toLowerCase().includes(text) || c.summary.toLowerCase().includes(text));
  if (q.categories.length) list = list.filter((c) => q.categories.includes(c.category ?? ""));
  if (q.updatedAfter) list = list.filter((c) => c.updatedAt >= q.updatedAfter!);
  if (q.minDownloads) list = list.filter((c) => c.downloads >= q.minDownloads!);
  if (q.maxSizeKb) list = list.filter((c) => (c.sizeBytes ?? 0) / 1024 <= q.maxSizeKb!);
  const key: Record<T.SortKey, (c: T.ModCard) => number | string> = {
    relevance: (c) => c.endorsements,
    downloads: (c) => c.downloads,
    endorsements: (c) => c.endorsements,
    updated: (c) => c.updatedAt,
    created: (c) => c.createdAt,
    size: (c) => c.sizeBytes ?? 0,
    name: (c) => c.name.toLowerCase(),
  };
  const k = key[q.sort];
  list = [...list].sort((a, b) => {
    const x = k(a), y = k(b);
    const r = x < y ? -1 : x > y ? 1 : 0;
    return q.ascending ? r : -r;
  });
  const cats = new Map<string, number>();
  list.forEach((c) => c.category && cats.set(c.category, (cats.get(c.category) ?? 0) + 1));
  const total = list.length;
  const pageCards = list.slice(q.offset, q.offset + q.count).map(decorate);
  const count = (f: (c: T.ModCard) => boolean) => pageCards.filter(f).length;
  const facets: T.Facets = {
    categories: [...cats.entries()].map(([value, count]) => ({ value, count })).sort((a, b) => b.count - a.count),
    tags: [],
    frameworks: (["cet", "red4ext", "redscript", "archiveXl", "tweakXl", "codeware"] as T.Framework[])
      .map((f) => ({ value: f, count: count((c) => c.derived.frameworks.includes(f)) }))
      .filter((f) => f.count > 0),
    kinds: (["archive", "script", "redmod", "framework"] as T.ModKind[]).map((k2) => ({ value: k2, count: count((c) => c.derived.kind === k2) })).filter((f) => f.count > 0),
    compatible: count((c) => c.compat === "updated" || c.compat === "likely"),
    downloaded: count((c) => c.downloaded),
    notDownloaded: count((c) => !c.downloaded),
    hidden: count((c) => c.hidden),
  };
  const before = pageCards.length;
  const mods = pageCards.filter(
    (c) =>
      (q.showHidden || !c.hidden) &&
      q.frameworks.every((f) => c.derived.frameworks.includes(f)) &&
      (q.kinds.length === 0 || (c.derived.kind && q.kinds.includes(c.derived.kind))) &&
      (!q.compatibleOnly || c.compat === "updated" || c.compat === "likely") &&
      (q.downloaded == null || c.downloaded === q.downloaded),
  );
  return { mods, total, offset: q.offset, facets, cached: false, filteredOut: before - mods.length };
}

function health(): T.HealthReport {
  const enabled = state.library.filter((m) => m.enabled);
  const needed = new Set(enabled.flatMap((m) => m.frameworks));
  const has = new Set(state.library.filter((m) => m.kind === "framework").map((m) => m.name));
  const fwItems: T.HealthItem[] = [];
  const names: Record<string, [string, number]> = { cet: ["Cyber Engine Tweaks", 107], red4ext: ["RED4ext", 2380], redscript: ["redscript", 1511], archiveXl: ["ArchiveXL", 4198], tweakXl: ["TweakXL", 4197], codeware: ["Codeware", 7780] };
  for (const [k, [n, id]] of Object.entries(names)) {
    if (has.has(n)) fwItems.push({ title: n, detail: "Installed.", status: "ok", fix: null });
    else if (needed.has(k as T.Framework))
      fwItems.push({ title: `${n} is missing`, detail: "Enabled mods need it.", status: "bad", fix: { action: { type: "installFramework", framework: k as T.Framework, modId: id }, label: `Install ${n}` } });
  }
  const cet = fwItems.find((i) => i.title === "Cyber Engine Tweaks");
  if (cet) {
    cet.title = "Cyber Engine Tweaks 1.33 may be outdated";
    cet.detail = "It was installed before the game's last update. Check for a newer version before playing.";
    cet.status = "warn";
    cet.fix = { action: { type: "updateFramework", framework: "cet", modId: 107 }, label: "Update Cyber Engine Tweaks" };
  }
  const mods: T.HealthModule[] = [
    { id: "game", title: "Game", status: "ok", items: [{ title: "Cyberpunk 2077 2.31", detail: state.settings.gameRoot ?? "", status: "ok", fix: null }] },
    { id: "frameworks", title: "Frameworks", status: "ok", items: fwItems },
    {
      id: "versions",
      title: "Versions",
      status: "warn",
      items: [{ title: "1 update(s) available", detail: "HUD Cleaner", status: "warn", fix: null }],
    },
    { id: "conflicts", title: "Conflicts", status: "warn", items: [{ title: "Rainy Night City and No Intro Videos both write basegame_shaders.archive", detail: "Rainy Night City wins (archive/pc/mod/basegame_shaders.archive).", status: "warn", fix: null }] },
    { id: "files", title: "Files", status: "ok", items: [] },
    {
      id: "redmod",
      title: "REDmod",
      status: state.redmodDirty ? "warn" : "ok",
      items: state.redmodDirty
        ? [{ title: "REDmods need deploying", detail: "REDmods or their load order changed since the last deploy.", status: "warn", fix: { action: { type: "runRedmodDeploy" }, label: "Deploy REDmods" } }]
        : [{ title: `${state.order.length} REDmod(s) deployed`, detail: "", status: "ok", fix: null }],
    },
  ];
  for (const m of mods) m.status = m.items.reduce<T.Status>((w, i) => (i.status === "bad" || w === "bad" ? "bad" : i.status === "warn" || w === "warn" ? "warn" : "ok"), "ok");
  const all = mods.flatMap((m) => m.items);
  const primaryFix = all.find((i) => i.status === "bad" && i.fix)?.fix ?? all.find((i) => i.status === "warn" && i.fix)?.fix ?? null;
  const worst = mods.some((m) => m.status === "bad") ? "bad" : mods.some((m) => m.status === "warn") ? "warn" : "ok";
  return { modules: mods, worst, primaryFix, problems: all.filter((i) => i.status !== "ok").length };
}

const sampleCollection: T.Collection = {
  info: { author: "curator", authorUrl: "", name: "Night City Refresh", description: "Visuals, vendors and a cleaner HUD.", domainName: "cyberpunk2077", gameVersions: ["2.31"] },
  mods: [107, 2380, 1511, 7780, 90001, 90003, 90008].map((id) => {
    const c = cards.find((x) => x.modId === id)!;
    return { name: c.name, version: c.version ?? "", optional: id === 90001, domainName: "cyberpunk2077", source: { type: "nexus", modId: id, fileId: id * 10, fileSize: c.sizeBytes } };
  }),
  modRules: [],
};

function precheck(c: T.Collection, includeOptional: boolean): T.PreCheck {
  const wanted = c.mods.filter((m) => includeOptional || !m.optional);
  const missing = wanted
    .filter((m) => !state.library.some((l) => l.modId === m.source.modId))
    .map((m) => ({ name: m.name, modId: m.source.modId ?? null, fileId: m.source.fileId ?? null, optional: m.optional, external: m.source.type !== "nexus", url: m.source.url ?? null, sizeBytes: m.source.fileSize ?? 0 }));
  return {
    name: c.info.name,
    total: c.mods.length,
    optional: c.mods.filter((m) => m.optional).length,
    installed: wanted.length - missing.length,
    missing,
    manualClicks: state.queue.premium ? missing.filter((m) => m.external).length : missing.length,
    external: missing.filter((m) => m.external).length,
    bytesToDownload: missing.reduce((s, m) => s + m.sizeBytes, 0),
    gameVersionOk: c.info.gameVersions.length ? c.info.gameVersions.includes("2.31") : null,
    gameVersions: c.info.gameVersions,
  };
}

const delay = <X>(v: X, ms = 120) => new Promise<X>((r) => setTimeout(() => r(v), ms));
const fail = (message: string, kind = "invalid") => Promise.reject({ kind, message });

export async function handle(cmd: string, a: Record<string, unknown> = {}): Promise<unknown> {
  const needKey = () => (!state.hasKey ? fail("Add your Nexus API key in Settings first.", "noKey") : null);
  switch (cmd) {
    case "status":
      return {
        hasKey: state.hasKey,
        account: state.account,
        game: state.settings.gameRoot == null ? null : { root: state.settings.gameRoot, launcher: "steam", version: "2.31", buildTime: BUILD, underProgramFiles: false, redmodTool: true },
        storeRoot: "D:\\PocketMods",
        rate: rate(),
        queue: summary(),
        updates: state.library.filter((m) => m.updateAvailable).length,
        profile: state.active,
        version: "0.1.0",
      } satisfies T.AppStatus;
    case "settings_get":
      return { ...state.settings };
    case "settings_save":
      state.settings = a.settings as T.Settings;
      return { ...state.settings };
    case "set_api_key": {
      const key = String(a.key ?? "").trim();
      if (!key) return fail("The key is empty.");
      state.hasKey = true;
      state.account = { userId: 1, name: "V", isPremium: key.toLowerCase().includes("premium"), isSupporter: false, profileUrl: null };
      state.queue.premium = state.account.isPremium;
      return delay(state.account, 400);
    }
    case "clear_api_key":
      state.hasKey = false;
      state.account = null;
      state.queue.premium = false;
      return null;
    case "refresh_account":
      return needKey() ?? state.account;
    case "nexus_probe":
      return needKey() ?? delay({ modTypeFields: ["modId", "name"], requestedButMissing: [], note: "Browser preview: no live call." });
    case "detect_games":
      return [{ root: state.settings.gameRoot, launcher: "steam", version: "2.31", buildTime: BUILD, underProgramFiles: false, redmodTool: true }];
    case "set_game_root":
      state.settings.gameRoot = String(a.path);
      return { root: a.path, launcher: "manual", version: "2.31", buildTime: BUILD, underProgramFiles: String(a.path).toLowerCase().includes("program files"), redmodTool: true };
    case "browse":
      return needKey() ?? delay(browse(a.query as T.BrowseQuery), 250);
    case "categories":
      return [...new Set(cards.map((c) => c.category))].map((name, i) => ({ id: i + 1, name, parent: null }));
    case "mod_detail":
      return needKey() ?? delay(detail(Number(a.modId)), 200);
    case "set_hidden":
      if (a.hidden) state.hidden.add(Number(a.modId));
      else state.hidden.delete(Number(a.modId));
      return null;
    case "endorse":
    case "set_tracked":
      return needKey() ?? delay(null);
    case "check_updates":
      return needKey() ?? delay(state.library.filter((m) => m.updateAvailable).length, 500);
    case "open_nexus":
      emitLocal("toast", { message: "In the app, the Nexus page opens in its own window." });
      return null;
    case "queue_view":
      return queueView();
    case "queue_mod": {
      if (!state.hasKey) return fail("Add your Nexus API key in Settings first.", "noKey");
      const c = cards.find((x) => x.modId === Number(a.modId));
      if (!c) return fail("Nexus has no such mod.", "notFound");
      const ids: number[] = [];
      for (const f of c.derived.frameworks) {
        const fwId = { cet: 107, red4ext: 2380, redscript: 1511, archiveXl: 4198, tweakXl: 4197, codeware: 7780, cybercmd: 5176 }[f];
        const fc = cards.find((x) => x.modId === fwId);
        if (fc && !state.library.some((m) => m.modId === fwId)) ids.push(enqueue(fc, fwId * 10, "dependency"));
      }
      ids.push(enqueue(c, Number(a.fileId ?? c.modId * 10), "browse"));
      pump();
      return ids;
    }
    case "queue_updates": {
      let n = 0;
      for (const m of state.library.filter((x) => x.updateAvailable)) {
        const c = cards.find((x) => x.modId === m.modId);
        if (c) {
          enqueue(c, m.updateAvailable!.fileId, "update");
          n++;
        }
      }
      pump();
      return n;
    }
    case "queue_action": {
      const id = a.id as number | null;
      const item = state.queue.items.find((i) => i.id === id);
      switch (a.action) {
        case "retry":
          if (item) {
            item.state = "waiting";
            item.error = null;
          }
          break;
        case "skip":
          if (item) {
            state.queue.items = state.queue.items.filter((i) => i !== item);
            item.state = "waiting";
            state.queue.items.push(item);
          }
          break;
        case "remove":
          state.queue.items = state.queue.items.filter((i) => i.id !== id);
          break;
        case "clear":
          state.queue.items = state.queue.items.filter((i) => i.state !== "done");
          break;
        case "pause":
          state.queue.paused = true;
          break;
        case "resume":
          state.queue.paused = false;
          break;
        case "open":
          // the person clicks "Mod Manager Download" on the page a moment later
          if (item && item.state === "needsClick") setTimeout(() => run(item), 1200);
          emitLocal("toast", { message: `Opened ${item?.modName ?? "the mod"} on Nexus. Click "Mod Manager Download" there.` });
          break;
      }
      pump();
      return null;
    }
    case "handle_nxm":
      return { type: "modFile", itemId: null };
    case "library":
      return state.library.map((m) => ({ ...m }));
    case "install_file": {
      const name = String(a.path).split(/[\\/]/).pop()!.replace(/\.(zip|7z|rar)$/i, "");
      const row: T.LibraryRow = { ...installedMod({ ...cards[6], name, modId: 0 }, 0), key: `local-${Date.now().toString(16).slice(-10)}`, modId: null, fileId: null };
      state.library.push(row);
      return delay({ key: row.key, name, kind: "archive", placed: 3, skippedDocs: 1, unplaced: [], conflicts: [], copied: 0, replaced: null, redmods: [] }, 500);
    }
    case "set_enabled": {
      const m = state.library.find((x) => x.key === a.key);
      if (m) {
        m.enabled = Boolean(a.enabled);
        m.deployed = m.enabled;
        if (m.redmods.length) state.redmodDirty = true;
      }
      return delay(null);
    }
    case "remove_mod":
      state.library = state.library.filter((x) => x.key !== a.key);
      return delay(null);
    case "redeploy":
      return delay(null);
    case "profiles":
      return { profiles: state.profiles, active: state.active };
    case "create_profile": {
      const name = String(a.name).trim();
      if (!name) return fail("Give the profile a name.");
      if (state.profiles.some((p) => p.toLowerCase() === name.toLowerCase())) return fail(`A profile called ${name} already exists.`);
      state.profiles.push(name);
      return null;
    }
    case "delete_profile":
      if (a.name === "Default") return fail("The Default profile can't be deleted.");
      if (a.name === state.active) return fail("Switch to another profile before deleting this one.");
      state.profiles = state.profiles.filter((p) => p !== a.name);
      return null;
    case "switch_profile":
      state.active = String(a.name);
      return delay(null, 300);
    case "load_order":
      return {
        rows: state.order.map((f) => ({ folder: f, name: f.replace(/([a-z])([A-Z])/g, "$1 $2"), version: "1.0", owner: state.library.find((m) => m.redmods.includes(f))?.name ?? null })),
        tool: true,
        dirty: state.redmodDirty,
        last: state.last,
      } satisfies T.LoadOrderView;
    case "set_load_order":
      state.order = a.order as string[];
      state.redmodDirty = true;
      return null;
    case "redmod_deploy":
      state.redmodDirty = false;
      state.last = { ok: true, problems: [], logTail: ["Deploying REDmods...", ...state.order.map((o) => `  ${o}: ok`), "Done."], at: now() };
      return delay(state.last, 900);
    case "health":
      return health();
    case "collections_browse":
      return needKey() ?? delay({
        collections: [
          { slug: "ncrefresh", name: "Night City Refresh", summary: "Visuals, vendors and a cleaner HUD.", author: "curator", thumbnailUrl: null, endorsements: 4100, downloads: 88_000, revision: 7, modCount: 7, sizeBytes: 1_400_000_000, adult: false, gameVersions: ["2.31"], category: "Visuals" },
          { slug: "lorefriendly", name: "Lore-Friendly Essentials", summary: "Small fixes that keep the tone.", author: "archivist", thumbnailUrl: null, endorsements: 2600, downloads: 51_000, revision: 12, modCount: 42, sizeBytes: 6_100_000_000, adult: false, gameVersions: ["2.3"], category: "Overhaul" },
        ],
        total: 2,
      });
    case "collection_fetch":
      return needKey() ?? delay({ collection: sampleCollection, check: precheck(sampleCollection, Boolean(a.includeOptional)) }, 600);
    case "collection_precheck":
      return precheck(a.collection as T.Collection, Boolean(a.includeOptional));
    case "collection_install": {
      const c = a.collection as T.Collection;
      const ids: number[] = [];
      for (const m of precheck(c, Boolean(a.includeOptional)).missing) {
        const card = cards.find((x) => x.modId === m.modId);
        if (card && m.fileId) ids.push(enqueue(card, m.fileId, `collection:${a.slug}`));
      }
      pump();
      return ids;
    }
    case "collection_import":
      return { collection: sampleCollection, check: precheck(sampleCollection, Boolean(a.includeOptional)) };
    case "collection_export": {
      const keys = a.keys as string[];
      if (!keys.length) return fail("Pick at least one mod for the collection.");
      const c: T.Collection = {
        info: { author: String(a.author), authorUrl: "", name: String(a.name), description: String(a.description), domainName: "cyberpunk2077", gameVersions: ["2.31"] },
        mods: state.library.filter((m) => keys.includes(m.key)).map((m) => ({ name: m.name, version: m.version ?? "", optional: false, domainName: "cyberpunk2077", source: { type: "nexus", modId: m.modId, fileId: m.fileId, md5: m.md5, fileSize: m.size } })),
        modRules: [],
      };
      state.saved = state.saved.filter((s) => s.name !== `local:${c.info.name}`);
      state.saved.unshift({ name: `local:${c.info.name}`, collection: c });
      return c;
    }
    case "saved_collections":
      return state.saved;
    case "delete_saved_collection":
      state.saved = state.saved.filter((s) => s.name !== a.name);
      return null;
    case "collection_publish":
      return state.settings.experiments.publishCollections
        ? fail("Publishing to Nexus isn't available: it's unconfirmed that a third-party app may create collections. Export the file and publish it from Vortex or the Nexus site for now.", "other")
        : fail("Publishing is an experiment; turn it on in Settings first.");
    case "modio_browse":
      if (!state.settings.experiments.modio) return fail("Turn on the mod.io experiment in Settings first.");
      return fail("Add Cyberpunk 2077's mod.io game ID in Settings.");
    case "modio_set_key":
      return null;
    case "nxm_status":
      return { supported: true, ours: state.handler.ours, command: state.handler.ours ? '"C:\\Program Files\\Pocket Mods\\Pocket Mods.exe" "%1"' : '"C:\\Program Files\\Vortex\\Vortex.exe" -d "%1"', holder: state.handler.ours ? "Pocket Mods.exe" : state.handler.holder, previous: state.handler.ours ? '"C:\\Program Files\\Vortex\\Vortex.exe" -d "%1"' : null } satisfies T.HandlerStatus;
    case "nxm_register":
      state.handler.ours = true;
      return handle("nxm_status");
    case "nxm_restore":
      state.handler.ours = false;
      return handle("nxm_status");
    case "app_update_check":
      return delay(null, 400);
    default:
      return fail(`The browser preview doesn't know "${cmd}".`, "other");
  }
}

/** For tests: puts the mock back to its first state where it matters. */
export function _setKey(premium: boolean) {
  state.hasKey = true;
  state.account = { userId: 1, name: "V", isPremium: premium, isSupporter: false, profileUrl: null };
  state.queue.premium = premium;
}
