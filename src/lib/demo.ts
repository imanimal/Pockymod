// Demo backend for running the UI in a plain browser (npm run dev). It answers the same commands as the Rust side
// with fixed data recorded from the live Nexus API (src/lib/demo/*.json, made by the demo_fixture example), so
// screens can be built and checked without Tauri. Nothing here talks to Nexus.
import { demoEmit, inTauri, registerDemo } from "./tauri";
import type { HealthReport, Profile, BrowseQuery, BrowseResult, CardView, DetailView, InstallRow, LocalFilter, NexusStatus, PathsView, QueueItem, QueueSnapshot, RateDisplay } from "./api";
import browseFixture from "./demo/browse.json";
import detailFixture from "./demo/details.json";
import collectionsFixture from "./demo/collections.json";
import checkFixture from "./demo/collection-check.json";

const rate: RateDisplay = { level: "unknown", remaining: null, limit: null, window: "hour", resetsAt: null, throttledUntil: null };
let status: NexusStatus = { hasKey: false, user: null, rateLimit: rate };
const flags = new Map<number, { hidden: boolean; ignored: boolean }>();
const store = new Map<string, string>();
const fixture = browseFixture as unknown as BrowseResult;
const details = detailFixture as unknown as DetailView[];

// a pretend queue: free account, each page "clicked" after a moment, then a fake download and install
const q: QueueItem[] = [];
const installs: InstallRow[] = [];
let qid = 0;
let pathsView: PathsView = { gameDir: null, libraryDir: null, defaultLibrary: null, allowCopy: false, gameLooksRight: false, sameVolume: null, inProgramFiles: false };
function snapshot(): QueueSnapshot {
  const open = q.filter((i) => i.state.state !== "done" && i.state.state !== "failed");
  const running = open.some((i) => i.state.state === "downloading" || i.state.state === "installing");
  return { items: q.map((i) => ({ ...i })), summary: {
    left: open.length, needsClick: open.filter((i) => i.state.state === "waiting" || i.state.state === "needsClick").length,
    bytesLeft: open.reduce((a, i) => a + i.size, 0), running, waitingOnUser: !running && open.some((i) => i.state.state === "needsClick"),
    failed: q.filter((i) => i.state.state === "failed").length, premium: false, bytesPerSec: running ? 4_200_000 : null,
    secondsLeft: running ? Math.round(open.reduce((a, i) => a + i.size, 0) / 4_200_000) : null } };
}
function pump() {
  demoEmit("queue", snapshot());
  if (q.some((i) => ["downloading", "installing", "needsClick"].includes(i.state.state))) return;
  const next = q.find((i) => i.state.state === "waiting");
  if (!next) return;
  next.state = { state: "needsClick" };
  demoEmit("queue", snapshot());
  setTimeout(() => {
    let got = 0;
    const tick = setInterval(() => {
      got = Math.min(next.size, got + Math.max(next.size / 6, 1));
      next.state = { state: "downloading", received: got, total: next.size };
      if (got >= next.size) {
        clearInterval(tick);
        next.state = { state: "done", installId: installs.length + 1, note: null };
        installs.push({ id: installs.length + 1, modId: next.modId, fileId: next.fileId, name: next.modName, version: next.version, modType: "archive", frameworks: [],
          staging: "", installedAt: Date.now() / 1000, updatedAt: null, deployedFiles: 3, enabled: true, loadOrder: installs.length });
        demoEmit("library-changed", null);
      }
      pump();
    }, 400);
  }, 2500);
}
function planFor(modId: number, fileId: number): QueueItem[] {
  const d = details.find((x) => x.detail.modId === modId);
  if (!d) throw { kind: "demo", message: "The demo data has details for the first eight mods only." };
  const f = d.files.find((x) => x.fileId === fileId) ?? d.files[0];
  const reqs = d.detail.modRequirements.nexusRequirements.nodes.filter((r) => !installs.some((i) => i.modId === r.modId))
    .map((r) => details.find((x) => x.detail.modId === r.modId)).filter((x): x is DetailView => !!x);
  const item = (dd: DetailView, ff: DetailView["files"][number], by: string | null): QueueItem => ({ id: 0, modId: dd.detail.modId, fileId: ff.fileId, modName: dd.detail.name,
    fileName: ff.name, version: ff.version, uri: ff.uri, size: ff.sizeInBytes ?? ff.size * 1024, requiredBy: by, manualOnly: false, state: { state: "waiting" } });
  return [...reqs.map((r) => item(r, r.files.find((x) => x.category === "MAIN") ?? r.files[0], d.detail.name)), item(d, f, null)];
}

let profilesList: Profile[] = [{ id: 1, name: "Default", active: true, enabled: 0 }];
function health(): HealthReport {
  const fw = (key: string, name: string, installed: boolean, version: string | null, latest: string, older = false, neededBy: string[] = []) =>
    ({ framework: key, key: key as never, name, installed, version, modified: null, olderThanGame: older, latest, updateAvailable: !!version && version !== latest, neededBy });
  return {
    game: pathsView.gameDir ? { path: pathsView.gameDir, exeFound: true, version: { file: "3.0.80.0", product: "3.0.80.0", productString: null }, detectedPatch: null,
      exeModified: null, redmodAvailable: true, frameworks: [], writable: true } : null,
    patch: "2.5", patchSource: "assumed",
    frameworks: [fw("cet", "Cyber Engine Tweaks", true, "1.37.1", "1.38.1", true, ["Native Settings UI"]), fw("red4ext", "RED4ext", true, "1.31.0", "1.31.0", false, ["ArchiveXL"]),
      fw("redscript", "redscript", false, null, "0.5.32", false, ["Mod Settings"]), fw("archivexl", "ArchiveXL", true, "1.28.6", "1.28.6"), fw("tweakxl", "TweakXL", false, null, "1.10.9"),
      fw("codeware", "Codeware", false, null, "1.19.2"), fw("cybercmd", "cybercmd", false, null, "0.0.9")],
    issues: [
      { id: "fw.missing.redscript", severity: "problem", area: "frameworks", title: "redscript is missing", detail: "Needed by Mod Settings. Without it those mods do nothing or stop the game from starting.", fix: { kind: "installMod", modId: 1511, name: "redscript" } },
      { id: "fw.old.cet", severity: "problem", area: "frameworks", title: "Cyber Engine Tweaks predates the game patch", detail: "The game was patched after Cyber Engine Tweaks was installed, and Nexus has a newer version (1.38.1). Update it before you play.", fix: { kind: "updateMod", modId: 107, name: "Cyber Engine Tweaks" } },
      { id: "redmod.deploy", severity: "warning", area: "redmod", title: "REDmods need deploying", detail: "REDmods were added, removed or reordered since the last REDmod deploy. The game will not see the change until it runs.", fix: { kind: "runRedmodDeploy" } },
      { id: "game.patch", severity: "info", area: "game", title: "Game patch assumed to be 2.5", detail: "The patch could not be read from the game. Pick it in Settings so compatibility checks are right.", fix: { kind: "openSettings" } },
    ],
    problems: 2, warnings: 1,
  };
}

function search(q: BrowseQuery, local: LocalFilter): BrowseResult {
  const text = (q.text ?? "").trim().toLowerCase();
  let nodes: CardView[] = fixture.nodes.map((n) => ({ ...n, ...(flags.get(n.modId) ?? {}) }));
  nodes = nodes.filter((n) =>
    (!text || `${n.name} ${n.summary}`.toLowerCase().includes(text))
    && (q.categories.length === 0 || q.categories.includes(n.category))
    && q.tags.every((t) => n.tags.some((x) => x.name === t))
    && !q.excludeTags.some((t) => n.tags.some((x) => x.name === t))
    && q.frameworks.every((f) => n.classification.frameworks.some((x) => x.key === f))
    && (!q.modType || n.classification.modType === q.modType)
    && (!q.updatedAfter || Date.parse(n.updatedAt) / 1000 >= q.updatedAfter)
    && (!q.compatibleSince || Date.parse(n.updatedAt) / 1000 >= q.compatibleSince)
    && (!q.downloadsMin || n.downloads >= q.downloadsMin)
    && (!q.sizeMaxKb || (n.fileSize ?? 0) <= q.sizeMaxKb));
  const before = nodes.length;
  nodes = nodes.filter((n) => (local.showHidden || !n.hidden) && local.downloaded !== true);
  const key: Record<string, (n: CardView) => number> = {
    downloads: (n) => n.downloads, endorsements: (n) => n.endorsements, updated: (n) => Date.parse(n.updatedAt),
    created: (n) => Date.parse(n.createdAt), size: (n) => n.fileSize ?? 0, relevance: () => 0, name: () => 0,
  };
  nodes.sort((a, b) => (key[q.sort](a) - key[q.sort](b)) * (q.ascending ? 1 : -1));
  return { ...fixture, total: nodes.length, offset: 0, nodes: nodes.slice(q.offset, q.offset + q.count), filteredOnPage: before - nodes.length, fromCache: false };
}

export function installDemo() {
  if (inTauri()) return;
  registerDemo({
    nexus_status: () => status,
    rate_limit: () => status.rateLimit,
    nexus_set_key: ({ key }) => {
      if (String(key).trim().length < 16) throw { kind: "invalid", message: "That does not look like a Nexus API key." };
      status = {
        hasKey: true,
        user: { userId: 1, name: "demo-user", isPremium: false, isSupporter: false },
        rateLimit: { level: "ok", remaining: 487, limit: 500, window: "hour", resetsAt: null, throttledUntil: null },
      };
      demoEmit("rate-limit", status.rateLimit);
      return status;
    },
    nexus_clear_key: () => { status = { hasKey: false, user: null, rateLimit: rate }; return status; },
    browse_search: ({ query, local }) => new Promise((ok) => setTimeout(() => ok(search(query as BrowseQuery, local as LocalFilter)), 120)),
    browse_detail: ({ modId }) => {
      const d = details.find((x) => x.detail.modId === modId);
      if (!d) throw { kind: "demo", message: "The demo data has details for the first eight mods only." };
      return { ...d, ...(flags.get(Number(modId)) ?? {}) };
    },
    set_mod_flag: ({ modId, flag, on }) => {
      const f = flags.get(Number(modId)) ?? { hidden: false, ignored: false };
      flags.set(Number(modId), { ...f, [String(flag)]: Boolean(on) });
    },
    settings_get: ({ key }) => store.get(String(key)) ?? null,
    settings_set: ({ key, value }) => { store.set(String(key), String(value)); },
    queue_get: () => snapshot(),
    queue_plan: ({ modId, fileId }) => planFor(Number(modId), Number(fileId)),
    queue_add: ({ modId, fileId }) => {
      for (const i of planFor(Number(modId), Number(fileId))) if (!q.some((x) => x.modId === i.modId && x.fileId === i.fileId)) q.push({ ...i, id: ++qid });
      pump();
      return snapshot();
    },
    queue_remove: ({ id }) => { const i = q.findIndex((x) => x.id === id); if (i >= 0) q.splice(i, 1); pump(); return snapshot(); },
    queue_retry: () => snapshot(),
    queue_clear_finished: () => { for (let i = q.length - 1; i >= 0; i--) if (["done", "failed"].includes(q[i].state.state)) q.splice(i, 1); demoEmit("queue", snapshot()); return snapshot(); },
    queue_show_page: () => {},
    open_nexus_page: () => { throw { kind: "demo", message: "Nexus pages open in their own window in the desktop app." }; },
    open_nexus_url: () => { throw { kind: "demo", message: "Nexus pages open in their own window in the desktop app." }; },
    library_list: () => installs,
    library_downloads: () => [],
    library_uninstall: ({ installId }) => { const i = installs.findIndex((x) => x.id === installId); if (i >= 0) installs.splice(i, 1); return { removed: 3, restored: [], leftAlone: [] }; },
    library_deploy: () => ({ outcome: "deployed", linked: 3, copied: 0, backedUp: [], tookFrom: [] }),
    library_install_file: () => { throw { kind: "demo", message: "Installing from a file needs the desktop app." }; },
    paths_get: () => pathsView,
    paths_set: ({ gameDir, libraryDir, allowCopy }) => {
      const g = (gameDir as string | undefined) ?? pathsView.gameDir;
      pathsView = { ...pathsView, gameDir: g, libraryDir: (libraryDir as string | undefined) ?? pathsView.libraryDir ?? (g ? `${g.slice(0, 2)}\\PocketMods` : null),
        defaultLibrary: g ? `${g.slice(0, 2)}\\PocketMods` : null, allowCopy: (allowCopy as boolean | undefined) ?? pathsView.allowCopy, gameLooksRight: !!g, sameVolume: g ? true : null,
        inProgramFiles: !!g && g.toLowerCase().includes("program files") };
      return pathsView;
    },
    game_detect: () => [{ store: "Steam", path: "D:\\SteamLibrary\\steamapps\\common\\Cyberpunk 2077", exists: true }],
    game_inspect: () => health().game,
    health_check: () => health(),
    redmod_deploy: () => ({ ok: true, mods: [], problems: [], log: "Deploying 0 mods\nDone" }),
    redmod_order: () => [],
    profiles_list: () => profilesList.map((p) => ({ ...p, enabled: p.active ? installs.length : 0 })),
    profile_create: ({ name }) => { const id = profilesList.length + 1; profilesList.push({ id, name: String(name), active: false, enabled: 0 }); return id; },
    profile_rename: () => {},
    profile_delete: ({ id }) => { profilesList = profilesList.filter((p) => p.id !== id || p.active); },
    profile_activate: ({ id }) => { profilesList = profilesList.map((p) => ({ ...p, active: p.id === id })); return { removed: [], deployed: [], notDeployed: [] }; },
    mod_set_enabled: ({ installId, on }) => { const i = installs.find((x) => x.id === installId); if (i) i.enabled = Boolean(on); return null; },
    load_order_set: ({ installIds }) => { (installIds as number[]).forEach((id, n) => { const i = installs.find((x) => x.id === id); if (i) i.loadOrder = n; }); installs.sort((a, b) => a.loadOrder - b.loadOrder); },
    updates_check: () => [],
    updates_last: () => [],
    queue_add_mod: () => snapshot(),
    app_update_check: () => ({ current: "0.1.0", latest: null, newer: false, notes: null, url: null, message: "No published release found yet." }),
    modio_has_key: () => false,
    modio_set_key: () => { throw { kind: "demo", message: "mod.io needs the desktop app." }; },
    modio_clear_key: () => {},
    modio_browse: () => { throw { kind: "invalid", message: "Add a mod.io API key in Settings first." }; },
    collections_browse: () => collectionsFixture,
    collection_check: () => new Promise((ok) => setTimeout(() => ok(checkFixture), 300)),
    collection_install: ({ check }) => {
      for (const e of (check as { entries: Array<{ modId: number; fileId: number; name: string; fileName: string; version: string; size: number; uri: string; status: string }> }).entries.filter((x) => x.status !== "installed").slice(0, 5))
        q.push({ id: ++qid, modId: e.modId, fileId: e.fileId, modName: e.name, fileName: e.fileName, version: e.version, uri: e.uri, size: e.size, requiredBy: "collection", manualOnly: false, state: { state: "waiting" } });
      pump();
      return snapshot();
    },
    local_collection_create: ({ name, description }) => {
      if (installs.length === 0) throw { kind: "invalid", message: "There are no Nexus mods enabled to put in a collection. Mods installed from a file cannot be shared." };
      return { format: "pocket-mods.collection", version: 1, name, description, game: "cyberpunk2077", gameVersion: "2.5", createdAt: new Date().toISOString(),
        mods: installs.map((i) => ({ modId: i.modId, fileId: i.fileId, name: i.name, fileName: i.name, version: i.version, md5: "d41d8cd98f00b204e9800998ecf8427e", size: 1000, optional: false, requires: [], uri: "" })),
        loadOrder: installs.map((i) => i.modId) };
    },
    local_collection_export: () => {},
    local_collection_import: () => { throw { kind: "demo", message: "Importing a file needs the desktop app." }; },
    nxm_handler_status: () => ({ supported: true, ours: false, owner: "Vortex", command: null }),
    nxm_handler_register: () => ({ supported: true, ours: true, owner: "Pocket Mods", command: null }),
    patches: () => [["2.0", "2023-09-21"], ["2.1", "2023-12-05"], ["2.12", "2024-03-01"], ["2.2", "2024-12-10"], ["2.3", "2025-07-17"], ["2.5", "2026-10-22"]]
      .map(([version, released]) => [{ version, released }, Date.parse(`${released}T00:00:00Z`) / 1000]),
  });
}
