// Demo backend for running the UI in a plain browser (npm run dev). It answers the same commands as the Rust side
// with fixed data recorded from the live Nexus API (src/lib/demo/*.json, made by the demo_fixture example), so
// screens can be built and checked without Tauri. Nothing here talks to Nexus.
import { demoEmit, inTauri, registerDemo } from "./tauri";
import type { ModManage, MyMods, HealthReport, Profile, BrowseQuery, BrowseResult, CardView, DetailView, InstallRow, LocalFilter, NexusStatus, PathsView, QueueItem, QueueSnapshot, RateDisplay } from "./api";
import browseFixture from "./demo/browse.json";
import detailFixture from "./demo/details.json";
import collectionsFixture from "./demo/collections.json";
import checkFixture from "./demo/collection-check.json";
import mediaFixture from "./demo/media.json";
import myModsFixture from "./demo/my-mods.json";
import myModFilesFixture from "./demo/my-mod-files.json";

// the marketplace folder in this repository, so the demo shows what the app would fetch
const marketFiles = import.meta.glob("../../marketplace/**/*.json", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

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
          staging: "", installedAt: Date.now() / 1000, updatedAt: null, deployedFiles: 3, enabled: true, loadOrder: installs.length,
          source: "pocket", category: details.find((d) => d.detail.modId === next.modId)?.detail.category ?? null, group: groupOf(details.find((d) => d.detail.modId === next.modId)?.detail.category ?? null), customGroup: false, author: details.find((d) => d.detail.modId === next.modId)?.detail.author ?? null });
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

const customGroups: string[] = [];
type DemoTrash = { id: number; row: InstallRow; reason: string; removedAt: number };
const demoTrash: DemoTrash[] = [];
let trashSeq = 0;
function toTrash(i: InstallRow, reason = "removed") { demoTrash.unshift({ id: ++trashSeq, row: { ...i }, reason, removedAt: Math.floor(Date.now() / 1000) }); }
function groupOf(c: string | null): string {
  const x = (c ?? "").toLowerCase();
  if (x.includes("preset")) return "Character presets";
  if (x === "modders resources" || x === "utilities") return "Core";
  if (x === "armour and clothing") return "Clothing";
  if (x === "appearance" || x === "characters") return "Appearance";
  if (x === "gameplay" || x === "scripts") return "Gameplay";
  if (x === "user interface") return "Interface";
  return "Other";
}
let profilesList: Profile[] = [{ id: 1, name: "Default", active: true, enabled: 0 }];
let demoBrowser = { open: false, popped: false, url: null as string | null, title: null, loading: false, reveal: false };
function demoOpenPage(url: string) {
  if (store.get("nexus.pages") === "pane") { demoBrowser = { ...demoBrowser, open: true, url, loading: false, reveal: true }; demoEmit("browser", demoBrowser); }
  else window.open(url, "_blank", "noopener,noreferrer");
}
/** Whether a mod is on in a profile that is not active, by "profile:install". */
const demoMember = new Map<string, boolean>();
const demoVortexProfiles = [{ name: "Default", on: 5, active: true }, { name: "Hardcore", on: 3, active: false }, { name: "Photo mode", on: 2, active: false }];
let demoProfilesIn = false;
function demoImportProfiles() {
  const vx = installs.filter((i) => i.source === "vortex");
  const out = demoVortexProfiles.map((vp) => {
    let p = profilesList.find((x) => x.name === vp.name);
    const created = !p;
    if (!p) { p = { id: profilesList.length + 1, name: vp.name, active: false, enabled: 0 }; profilesList.push(p); }
    if (!p.active) vx.forEach((i, k) => demoMember.set(`${p!.id}:${i.id}`, k < vp.on));
    return { name: vp.name, created, on: Math.min(vp.on, vx.length), of: vx.length };
  });
  demoProfilesIn = true;
  return { profiles: out, active: "Default", deployed: 0, removed: 0, notDeployed: [] };
}
function demoScan() {
  return { vortex: { staging: "%APPDATA%\\Vortex\\cyberpunk2077\\mods", mods: 6, deployed: 5, alreadyTracked: installs.filter((i) => i.source === "vortex").length ? 6 : 0,
      sample: fixture.nodes.slice(8, 14).map((n) => n.name), profiles: demoVortexProfiles }, found: installs.some((i) => i.source === "found") ? [] : [{ name: "Hand-placed hair", kind: "archive", files: ["archive/pc/mod/hair.archive"], modId: null }] };
}
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
    && (!q.createdAfter || Date.parse(n.createdAt) / 1000 >= q.createdAfter)
    && (!q.createdBefore || Date.parse(n.createdAt) / 1000 <= q.createdBefore)
    && (!q.endorsementsMin || n.endorsements >= q.endorsementsMin)
    && (!q.downloadsMax || n.downloads <= q.downloadsMax)
    && (!q.compatibleSince || Date.parse(n.updatedAt) / 1000 >= q.compatibleSince)
    && (!q.downloadsMin || n.downloads >= q.downloadsMin)
    && (!q.sizeMaxKb || (n.fileSize ?? 0) <= q.sizeMaxKb));
  const before = nodes.length;
  nodes = nodes.filter((n) => (local.showHidden || !n.hidden) && local.downloaded !== true);
  const key: Record<string, (n: CardView) => number> = {
    downloads: (n) => n.downloads, endorsements: (n) => n.endorsements, updated: (n) => Date.parse(n.updatedAt),
    created: (n) => Date.parse(n.createdAt), size: (n) => n.fileSize ?? 0, relevance: () => 0, name: () => 0,
    random: (n) => ((n.modId * 2654435761 + (q.seed ?? 1) * 40503) >>> 0) % 1000,
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
    mod_endorse: () => { if (!status.hasKey) throw { kind: "no_key", message: "Add your Nexus API key in Settings first." }; },
    mod_track: () => { if (!status.hasKey) throw { kind: "no_key", message: "Add your Nexus API key in Settings first." }; },
    updates_skip: () => { demoEmit("updates", []); return []; },
    queue_cancel: ({ id }) => { const k = q.findIndex((x) => x.id === id); if (k >= 0) q[k].state = { state: "failed", message: "Download cancelled. Retry picks it up where it stopped." }; pump(); return snapshot(); },
    queue_clear_finished: () => { for (let i = q.length - 1; i >= 0; i--) if (["done", "failed"].includes(q[i].state.state)) q.splice(i, 1); demoEmit("queue", snapshot()); return snapshot(); },
    queue_show_page: () => {},
    // pages open in this browser (a new tab), or in the stand-in pane (the desktop app shows the real page there)
    open_nexus_page: ({ modId, fileId }) => demoOpenPage(`https://www.nexusmods.com/cyberpunk2077/mods/${modId}${fileId ? `?tab=files&file_id=${fileId}&nmm=1` : ""}`),
    open_nexus_url: ({ url }) => demoOpenPage(String(url)),
    browser_state: () => demoBrowser,
    background_state: () => ({ readyVersion: null, modsReady: 0 }),
    background_check: () => ({ readyVersion: null, modsReady: 0 }),
    app_update_install: () => { throw { kind: "demo", message: "Updates install in the desktop app." }; },
    autostart_get: () => store.get("demo.autostart") === "1",
    autostart_set: ({ on }) => { store.set("demo.autostart", on ? "1" : "0"); return Boolean(on); },
    browser_bounds: () => {},
    browser_nav: () => {},
    browser_go: ({ url }) => demoOpenPage(String(url)),
    browser_close: () => { demoBrowser = { ...demoBrowser, open: false, popped: false, url: null, reveal: false }; demoEmit("browser", demoBrowser); },
    browser_pop: ({ out }) => { demoBrowser = { ...demoBrowser, open: true, popped: Boolean(out), reveal: true }; demoEmit("browser", demoBrowser); },
    browser_external: () => { if (demoBrowser.url) window.open(demoBrowser.url, "_blank", "noopener,noreferrer"); },
    browser_extensions_folder: () => ["%APPDATA%\\app.pocketmods.desktop\\browser-extensions", 0],
    library_list: () => installs,
    library_downloads: () => installs.filter((i) => i.source !== "found").map((i) => ({ modId: i.modId, fileId: i.fileId, name: i.name, version: i.version, path: "", installed: true, md5: "",
      size: (fixture.nodes.find((n) => n.modId === i.modId)?.fileSize ?? 900) * 1024 })),
    library_uninstall: ({ installId }) => { const i = installs.findIndex((x) => x.id === installId); if (i >= 0) { toTrash(installs[i]); installs.splice(i, 1); } return { removed: 3, restored: [], leftAlone: [] }; },
    library_trash_list: () => demoTrash.map((t) => ({ id: t.id, installId: t.row.id, name: t.row.name, modId: t.row.modId, version: t.row.version, source: t.row.source, reason: t.reason, size: 18_400_000, removedAt: t.removedAt })),
    library_trash_restore: ({ trashIds }) => {
      const out: number[] = [];
      for (const id of trashIds as number[]) {
        const k = demoTrash.findIndex((t) => t.id === id);
        if (k < 0) { out.push(0); continue; }
        installs.push(demoTrash[k].row); out.push(demoTrash[k].row.id); demoTrash.splice(k, 1);
      }
      installs.sort((a, b) => a.loadOrder - b.loadOrder);
      demoEmit("library-changed", null);
      return out;
    },
    library_trash_purge: ({ trashId, all }) => { const before = demoTrash.length; for (let k = demoTrash.length - 1; k >= 0; k--) if (all || demoTrash[k].id === trashId) demoTrash.splice(k, 1); return before - demoTrash.length; },
    library_duplicates: () => {
      const by = new Map<number, InstallRow[]>();
      for (const i of installs.filter((x) => x.modId < 900_000_000)) by.set(i.modId, [...(by.get(i.modId) ?? []), i]);
      return [...by.values()].filter((v) => v.length > 1).map((v) => v.sort((a, b) => Number(b.enabled) - Number(a.enabled) || Number(!!a.origin) - Number(!!b.origin) || Number(a.source === "pocket") - Number(b.source === "pocket")).map((x) => x.id));
    },
    library_keep_one: ({ keep, remove }) => { for (const id of remove as number[]) { const k = installs.findIndex((x) => x.id === id && id !== keep); if (k >= 0) { toTrash(installs[k], "extra copy"); installs.splice(k, 1); } } demoEmit("library-changed", null); },
    library_restore_replaced: ({ installIds }) => { let n = 0; for (const i of installs) if ((installIds as number[]).includes(i.id) && i.replaced) { i.replaced = 0; i.source = "found"; i.origin = null; n++; } demoEmit("library-changed", null); return n; },
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
    mod_set_enabled: ({ installId, on }) => { const i = installs.find((x) => x.id === installId); if (i) i.enabled = Boolean(on); demoEmit("library-changed", null); return null; },
    profile_memberships: ({ installId }) => [...profilesList].sort((a, b) => Number(b.active) - Number(a.active)).map((p) => ({
      profileId: p.id, name: p.name, active: p.active,
      enabled: p.active ? !!installs.find((x) => x.id === installId)?.enabled : demoMember.get(`${p.id}:${installId}`) ?? false })),
    profile_mod_set: ({ profileId, installId, on }) => {
      const p = profilesList.find((x) => x.id === profileId);
      if (p?.active) { const i = installs.find((x) => x.id === installId); if (i) i.enabled = Boolean(on); } else demoMember.set(`${profileId}:${installId}`, Boolean(on));
      demoEmit("library-changed", null); return null;
    },
    load_order_set: ({ installIds }) => { (installIds as number[]).forEach((id, n) => { const i = installs.find((x) => x.id === id); if (i) i.loadOrder = n; }); installs.sort((a, b) => a.loadOrder - b.loadOrder); },
    updates_check: () => [],
    updates_last: () => installs.filter((i) => i.source === "vortex").slice(0, 2).map((i) => ({ modId: i.modId, name: i.name, installed: i.version, latest: `${i.version}.1`, updatedAt: "2026-11-01T00:00:00Z" })),
    queue_add_mod: () => snapshot(),
    downloads_usage: () => ({ files: 0, bytes: 0, installedFiles: 0, installedBytes: 0 }),
    downloads_cleanup: () => 0,
    nxm_handler_test: () => { throw { kind: "demo", message: "The link test needs the desktop app on Windows." }; },
    game_launch: () => { throw { kind: "demo", message: "Starting the game needs the desktop app." }; },
    game_launch_vanilla: () => { throw { kind: "demo", message: "Starting the game needs the desktop app." }; },
    game_vanilla_pending: () => false,
    scripts_reset: () => 2,
    library_identify: ({ path }) => {
      const stem = String(path).split(/[\\/]/).pop()!.replace(/\.(zip|7z|rar)$/i, "");
      const m = /^(.+?)-(\d+)-([\d-]+)-(\d{9,11})$/.exec(stem);
      return m ? { modId: Number(m[2]), fileId: null, name: m[1].replace(/[_-]/g, " "), version: m[3].replace(/-/g, "."), fileName: null } : { modId: null, fileId: null, name: stem.replace(/[_-]/g, " "), version: "", fileName: null };
    },
    game_vanilla_restore: () => 0,
    game_open_folder: () => { throw { kind: "demo", message: "Opening folders needs the desktop app." }; },
    app_update_check: () => ({ current: "0.1.0", latest: null, newer: false, notes: null, url: null, message: "No published release found yet." }),
    modio_has_key: () => false,
    modio_set_key: () => { throw { kind: "demo", message: "mod.io needs the desktop app." }; },
    modio_clear_key: () => {},
    modio_browse: () => { throw { kind: "invalid", message: "Add a mod.io API key in Settings first." }; },
    collections_browse: () => collectionsFixture,
    media_browse: ({ kind }) => (mediaFixture as Record<string, unknown>)[String(kind)],
    collection_check: () => new Promise((ok) => setTimeout(() => ok(checkFixture), 300)),
    collection_detail: ({ slug }) => {
      const c = collectionsFixture.nodes.find((n) => n.slug === slug);
      return { headerImage: c?.tileImage?.url ?? null, description: c ? `${c.summary}\n\n[b]What is in it[/b]\nEvery mod in this collection is listed below, with what you already have. Install adds the rest to the download queue.\n\n[img]${c.tileImage?.url ?? ""}[/img]` : null };
    },
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
    existing_scan: async () => {
      for (const d of ["%APPDATA%\\Vortex", "archive\\pc\\mod", "mods", "r6\\scripts", "r6\\tweaks", "bin\\x64\\plugins\\cyber_engine_tweaks\\mods", "red4ext\\plugins"]) {
        demoEmit("existing-progress", { stage: d.startsWith("%") ? "vortex" : "folder", done: 0, total: 0, name: d.startsWith("%") ? d : `D:\\SteamLibrary\\steamapps\\common\\Cyberpunk 2077\\${d}` });
        await new Promise((r) => setTimeout(r, 140));
      }
      return demoScan();
    },
    vortex_profiles_scan: () => ({ profiles: demoVortexProfiles, imported: demoProfilesIn, tracked: installs.filter((i) => i.source === "vortex").length }),
    vortex_profiles_import: () => demoImportProfiles(),
    existing_track: async ({ vortexMods, foundMods, vortexProfiles }) => {
      let n = 0;
      if (vortexMods) {
        const names = fixture.nodes.slice(8, 14).map((x) => x.name);
        for (let k = 0; k < names.length; k++) { demoEmit("existing-progress", { stage: "matching", done: k + 1, total: names.length, name: names[k] }); await new Promise((r) => setTimeout(r, 260)); }
        demoEmit("existing-progress", { stage: "tracking", done: names.length, total: names.length, name: "" });
      }
      if (foundMods) { demoEmit("existing-progress", { stage: "linking", done: 0, total: 0, name: "" }); await new Promise((r) => setTimeout(r, 300)); }
      if (vortexMods) for (const c of fixture.nodes.slice(8, 14)) {
        installs.push({ id: installs.length + 1, modId: c.modId, fileId: 1, name: c.name, version: c.version, modType: c.classification.modType, frameworks: c.classification.frameworks.map((f) => f.key),
          staging: "", installedAt: 0, updatedAt: null, deployedFiles: 4, enabled: n !== 5, loadOrder: installs.length, source: "vortex", category: c.category, group: groupOf(c.category), customGroup: false, picture: c.thumbnailUrl?.replace("/thumbnails/", "/"), author: c.author });
        n++;
      }
      if (vortexMods) {
        // what the old behaviour left behind, so the clean-up notices have something to show
        const v = installs.find((i) => i.source === "vortex");
        if (v) installs.push({ ...v, id: installs.length + 1, source: "pocket", enabled: false, origin: "Better Netrunning", loadOrder: installs.length });
        installs.push({ id: installs.length + 1, modId: 4198, fileId: 77, name: "ArchiveXL", version: "1.21.1", modType: "framework", frameworks: [], staging: "", installedAt: 0, updatedAt: null,
          deployedFiles: 3, enabled: true, loadOrder: installs.length, source: "pocket", category: "Modders Resources", group: "Core", customGroup: false, origin: "Hand-placed hair", replaced: 3 });
      }
      if (foundMods) installs.push({ id: installs.length + 1, modId: 900000001, fileId: 1, name: "Hand-placed hair", version: "", modType: "archive", frameworks: [], staging: "", installedAt: 0, updatedAt: null,
        deployedFiles: 1, enabled: true, loadOrder: installs.length, source: "found", category: null, group: "Other", customGroup: false });
      const profiles = vortexMods && vortexProfiles !== false ? demoImportProfiles() : null;
      demoEmit("library-changed", null);
      return { vortex: { tracked: vortexMods ? 6 : 0, matchedOnNexus: vortexMods ? 6 : 0, already: 0 }, found: foundMods ? 1 : 0, profiles };
    },
    groups_list: () => ({ presets: ["Core", "Clothing", "Character presets", "Appearance", "Gameplay", "Visuals and places", "Weapons and vehicles", "Audio", "Interface", "Other"], custom: customGroups }),
    groups_set: ({ installIds, group }) => {
      if (group && !customGroups.includes(String(group))) customGroups.push(String(group));
      for (const i of installs) if ((installIds as number[]).includes(i.id)) { i.group = group ? String(group) : groupOf(i.category); i.customGroup = !!group; }
      demoEmit("library-changed", null);
    },
    group_create: ({ name }) => { if (!customGroups.includes(String(name))) customGroups.push(String(name)); },
    group_rename: ({ from, to }) => { const k = customGroups.indexOf(String(from)); if (k >= 0) customGroups[k] = String(to); for (const i of installs) if (i.group === from) i.group = String(to); },
    group_delete: ({ name }) => { const k = customGroups.indexOf(String(name)); if (k >= 0) customGroups.splice(k, 1); for (const i of installs) if (i.group === name) { i.group = groupOf(i.category); i.customGroup = false; } },
    library_bulk: ({ installIds, action }) => {
      const ids = installIds as number[];
      for (const i of [...installs]) if (ids.includes(i.id)) { if (action === "uninstall") { toTrash(i); installs.splice(installs.indexOf(i), 1); } else i.enabled = action === "enable" || action === "enable-win"; }
      demoEmit("library-changed", null);
      return { done: ids.length, failed: [] };
    },
    nxm_handler_status: () => ({ supported: true, ours: false, owner: "Vortex", command: null }),
    nxm_handler_register: () => ({ supported: true, ours: true, owner: "Pockymod", command: null }),
    addon_list: ({ kind }) => { try { return Object.values(JSON.parse(localStorage.getItem(`pm.demo.addons.${String(kind)}`) ?? "{}")); } catch { return []; } },
    addon_save: ({ kind, id, json }) => {
      const k = `pm.demo.addons.${String(kind)}`;
      let all: Record<string, string> = {};
      try { all = JSON.parse(localStorage.getItem(k) ?? "{}"); } catch { /* start over */ }
      all[String(id)] = String(json);
      try { localStorage.setItem(k, JSON.stringify(all)); } catch { /* storage unavailable */ }
    },
    addon_remove: ({ kind, id }) => {
      const k = `pm.demo.addons.${String(kind)}`;
      try { const all = JSON.parse(localStorage.getItem(k) ?? "{}"); delete all[String(id)]; localStorage.setItem(k, JSON.stringify(all)); } catch { /* storage unavailable */ }
    },
    market_get: ({ path }) => {
      const hit = marketFiles[`../../marketplace/${String(path)}`];
      if (hit === undefined) throw { kind: "http", message: `The marketplace has no ${String(path)} yet.` };
      return hit;
    },
    // ?market=offline in the address shows the marketplace as an old cached copy
    market_fetch: ({ path }) => {
      const hit = marketFiles[`../../marketplace/${String(path)}`];
      if (hit === undefined) throw { kind: "http", message: `The marketplace has no ${String(path)} yet.` };
      return { text: hit, source: location.search.includes("market=offline") ? "offline" : "network" };
    },
    ext_states: () => { try { return JSON.parse(localStorage.getItem("pm.demo.extState") ?? "{}"); } catch { return {}; } },
    ext_state_set: ({ id, state }) => {
      let all: Record<string, unknown> = {};
      try { all = JSON.parse(localStorage.getItem("pm.demo.extState") ?? "{}"); } catch { /* start over */ }
      if (state) all[String(id)] = state; else delete all[String(id)];
      try { localStorage.setItem("pm.demo.extState", JSON.stringify(all)); } catch { /* storage unavailable */ }
    },
    ext_tool_start: () => { throw { kind: "demo", message: "Starting programs needs the desktop app." }; },
    ext_launch: () => { throw { kind: "demo", message: "Starting the game needs the desktop app." }; },
    ext_open_link: () => { throw { kind: "demo", message: "Links open in your browser from the desktop app." }; },
    open_external: ({ url }) => {
      const u = String(url);
      if (!["https://github.com/imanimal/pocket-mods/", "https://www.nexusmods.com/"].some((p) => u.startsWith(p))) throw { kind: "invalid", message: "Only this project's GitHub pages and Nexus Mods pages open from here." };
      window.open(u, "_blank", "noopener,noreferrer");
    },
    text_file_open: () => null,
    text_file_save_as: () => false,
    patches: () => [["2.0", "2023-09-21"], ["2.1", "2023-12-05"], ["2.12", "2024-03-01"], ["2.2", "2024-12-10"], ["2.3", "2025-07-17"], ["2.5", "2026-10-22"]]
      .map(([version, released]) => [{ version, released }, Date.parse(`${released}T00:00:00Z`) / 1000]),
  });
  installTroubleshootDemo();
}

// ── logs, overlapping archives, standing choices, installer options, moving the library (demo answers) ──

type DemoRename = { modId: number; name: string; target: string; renamed: string; installId: number | null; createdAt: number };
const demoRenames: DemoRename[] = [];
let demoWins = [{ id: 1, winner: "Better Netrunning", loser: "Netrunner Rework", winnerInstall: 0, loserInstall: 0, files: 3, createdAt: Math.floor(Date.now() / 1000) - 86400 * 3 }];
/** Names for the demo's logs and archives: the user's demo installs first, then mods from the browse fixture. */
function demoNames(n: number): Array<{ id: number | null; modId: number | null; name: string; enabled: boolean }> {
  const own = installs.map((i) => ({ id: i.id as number | null, modId: i.modId as number | null, name: i.name, enabled: i.enabled }));
  const extra = fixture.nodes.slice(20, 20 + n).map((c) => ({ id: null, modId: c.modId as number | null, name: c.name, enabled: true }));
  return [...own, ...extra].slice(0, n);
}
function demoLogs() {
  const now = Math.floor(Date.now() / 1000);
  const start = now - 17 * 60;
  const [a, b, c] = demoNames(3);
  const t = (s: number) => new Date((start + s) * 1000).toISOString().replace("T", " ").slice(0, 19);
  const entry = (tool: string, log: string, level: "error" | "warning", text: string, who: { id: number | null } | undefined, subject: string | null, s = 4) =>
    ({ log, tool, level, time: t(s), text, subject, installId: who?.id ?? null });
  const rs = "r6/logs/redscript_rCURRENT.log";
  const axl = "red4ext/plugins/ArchiveXL/ArchiveXL-2026-11-04-11-52-10.log";
  const cet = "bin/x64/plugins/cyber_engine_tweaks/mods/HUDTweaks/HUDTweaks.log";
  const mods = [
    { who: a, entries: [
      entry("redscript", rs, "error", "error[UNRESOLVED_REF]: 'NetrunnerHelper' is not defined\n --> r6\\scripts\\BetterNetrunning\\Hacks.reds:212:9\n    |\n212 |     NetrunnerHelper.Boost(this);", a, "r6/scripts/betternetrunning/hacks.reds"),
      entry("redscript", rs, "error", "error[UNRESOLVED_METHOD]: 'GetQuickhackCost' is not defined on 'ScriptableSystem'\n --> r6\\scripts\\BetterNetrunning\\Cost.reds:48:5", a, "r6/scripts/betternetrunning/cost.reds"),
    ] },
    { who: b, entries: [entry("ArchiveXL", axl, "error", "HairRework.archive.xl: Can't resolve appearance \"hair_long_02\".", b, "hairrework.archive.xl", 6)] },
    { who: c, entries: [entry("CET mod HUDTweaks", cet, "warning", "[string \"init.lua\"]:88: attempt to index a nil value (field 'settings')\nstack traceback:\n  init.lua:88: in function 'onDraw'", c, "bin/x64/plugins/cyber_engine_tweaks/mods/hudtweaks/", 40)] },
  ].filter((m) => m.who).map((m) => ({ installId: m.who!.id ?? 0, name: m.who!.name, enabled: m.who!.enabled,
    errors: m.entries.filter((e) => e.level === "error").length, warnings: m.entries.filter((e) => e.level === "warning").length, entries: m.entries }));
  const other = [entry("RED4ext", "red4ext/logs/red4ext-2026-11-04-11-52-08.log", "warning", "mod_settings (version: 0.2.9) uses RED4ext.SDK v0.4.0 which is not supported by RED4ext v1.31.0.", undefined, null, 2)];
  const logsList = [
    { tool: "redscript", path: rs, written: start + 5, current: true, errors: 2, warnings: 0, size: 48_210 },
    { tool: "ArchiveXL", path: axl, written: start + 30, current: true, errors: 1, warnings: 0, size: 9_800 },
    { tool: "RED4ext", path: "red4ext/logs/red4ext-2026-11-04-11-52-08.log", written: start + 3, current: true, errors: 0, warnings: 1, size: 6_400 },
    { tool: "Cyber Engine Tweaks", path: "bin/x64/plugins/cyber_engine_tweaks/cyber_engine_tweaks.log", written: start + 60, current: true, errors: 0, warnings: 0, size: 3_200 },
    { tool: "CET mod HUDTweaks", path: cet, written: start + 40, current: true, errors: 0, warnings: 1, size: 1_100 },
    { tool: "TweakXL", path: "red4ext/plugins/TweakXL/TweakXL-2026-11-04-11-52-10.log", written: start + 12, current: true, errors: 0, warnings: 0, size: 2_400 },
    { tool: "Codeware", path: "red4ext/plugins/Codeware/Codeware.log", written: now - 86400 * 9, current: false, errors: 1, warnings: 0, size: 700 },
  ];
  const errors = mods.reduce((n, m) => n + m.errors, 0);
  const warnings = mods.reduce((n, m) => n + m.warnings, 0) + other.length;
  return { lastStart: start, logs: logsList, mods, other, errors, warnings, scriptsFailed: mods.some((m) => m.entries.some((e) => e.tool === "redscript")) };
}
function demoOverlaps() {
  const [a, b, c] = demoNames(3);
  const side = (who: { id: number | null; modId: number | null; name: string } | undefined, archives: string[], redmod = false) =>
    ({ installId: who?.id ?? null, modId: who?.modId ?? null, name: who?.name ?? archives[0].split("/").pop()!, archives, redmod });
  const renamed = (target: string) => demoRenames.some((r) => r.target === target);
  const out = [];
  if (a && !renamed("archive/pc/mod/HairRework.archive"))
    out.push({ winner: side({ id: null, modId: null, name: "#Hair Pack Base" }, ["archive/pc/mod/#HairPackBase.archive"]), loser: side(a, ["archive/pc/mod/HairRework.archive"]), files: 42,
      loadFirst: [{ target: "archive/pc/mod/HairRework.archive", name: "!HairRework.archive" }], how: "Archives load in name order and the first one wins." });
  if (b) out.push({ winner: side(b, ["archive/pc/mod/ApartmentLights.archive"]), loser: side(c, ["mods/BetterApartments/archives/apartments.archive"], true), files: 7, loadFirst: null,
    how: `${c?.name ?? "This mod"} is a REDmod. REDmod archives load after every archive in archive/pc/mod, so it cannot win by a new name.` });
  out.push({ winner: side(c ?? a, ["archive/pc/mod/basegame_ui_tweaks.archive"]), loser: side(undefined, ["archive/pc/mod/zz_old_ui.archive"]), files: 3, loadFirst: null,
    how: "zz_old_ui.archive is not managed by Pockymod. To make it win, rename it in archive/pc/mod so its name sorts first, for example with ! in front." });
  return { archives: 128, overlaps: out, sameContent: 13, unreadable: [], modlist: null, millis: 37 };
}
const demoFomod = {
  name: "Netrunner Suit Recolours", root: "",
  steps: [
    { name: "Colour", groups: [
      { name: "Suit colour", kind: "SelectExactlyOne", plugins: [
        { name: "Black", description: "The original black with cyan trim.", image: null, kind: "Optional" },
        { name: "Red", description: "Arasaka red with gold trim.", image: null, kind: "Recommended" },
        { name: "White", description: "Clean white with grey panels.", image: null, kind: "Optional" } ] },
      { name: "Extras", kind: "SelectAny", plugins: [
        { name: "Matching gloves", description: "Gloves in the same colour.", image: null, kind: "Optional" },
        { name: "Hood", description: "Adds the hood as a separate item.", image: null, kind: "Optional" } ] } ] },
    { name: "Fit", groups: [
      { name: "Body", kind: "SelectExactlyOne", plugins: [
        { name: "Vanilla body", description: "", image: null, kind: "Optional" }, { name: "Hyst EBB", description: "For the Hyst EBB body mod.", image: null, kind: "Optional" } ] } ] },
  ],
};
let demoChoices: Array<[number, number, number]> = [[0, 0, 1], [0, 1, 0], [1, 0, 0]];

function installTroubleshootDemo() {
  registerDemo({
    library_list: () => installs.map((i) => ({ ...i, hasOptions: i.source === "pocket" && i.modType !== "framework" })),
    logs_scan: () => new Promise((ok) => setTimeout(() => ok(demoLogs()), 150)),
    logs_report: () => {
      const l = demoLogs();
      return [`Pockymod 0.1.0 report`, `Game: Cyberpunk 2077 patch 2.5 (exe 3.0.80.0)`, `Frameworks: Cyber Engine Tweaks 1.37.1, RED4ext 1.31.0, ArchiveXL 1.28.6`,
        `Mods: ${installs.filter((i) => i.enabled).length} on of ${installs.length} installed`, `Errors: ${l.errors}, warnings: ${l.warnings}`,
        ...l.mods.flatMap((m) => [``, `${m.name} (${m.enabled ? "on" : "off"}): ${m.errors} errors, ${m.warnings} warnings`, ...m.entries.map((e) => `  [${e.tool}] ${e.level}: ${e.text.split("\n").slice(0, 2).join(" / ")}`)])].join("\n");
    },
    log_open: () => { throw { kind: "demo", message: "Opening logs needs the desktop app." }; },
    archive_overlaps: () => new Promise((ok) => setTimeout(() => ok(demoOverlaps()), 120)),
    archive_load_first: ({ installId, renames }) => {
      const who = installs.find((i) => i.id === installId);
      for (const r of renames as Array<{ target: string; name: string }>) demoRenames.unshift({ modId: who?.modId ?? 0, name: who?.name ?? "Mod", target: r.target, renamed: r.name, installId: (installId as number) ?? null, createdAt: Math.floor(Date.now() / 1000) });
      demoEmit("library-changed", null);
      return { outcome: "deployed", linked: 1, copied: 0, backedUp: [], tookFrom: [] };
    },
    archive_rename_undo: ({ modId, target }) => { const k = demoRenames.findIndex((r) => r.modId === modId && r.target === target); if (k >= 0) demoRenames.splice(k, 1); demoEmit("library-changed", null); },
    modlist_disable: () => true,
    rules_list: () => ({ wins: demoWins, renames: demoRenames }),
    rule_forget: ({ id }) => { demoWins = demoWins.filter((w) => w.id !== id); },
    fomod_options: ({ installId }) => ({ installId, name: installs.find((i) => i.id === installId)?.name ?? "Mod", fomod: demoFomod, choices: demoChoices, saved: true }),
    fomod_change: ({ choices }) => { demoChoices = choices as Array<[number, number, number]>; demoEmit("library-changed", null); return { outcome: "deployed", linked: 4, copied: 0, backedUp: [], tookFrom: [] }; },
    library_move_check: ({ to }) => {
      const from = pathsView.libraryDir ?? "D:\\PocketMods";
      const dest = String(to);
      const drive = (p: string) => p.slice(0, 2).toLowerCase();
      const problem = dest.toLowerCase().startsWith(from.toLowerCase()) ? "The new place is inside the library itself. Choose a folder outside it." : null;
      return { from, to: dest, sameDriveAsGame: drive(dest) === drive(pathsView.gameDir ?? from), quick: drive(dest) === drive(from), files: 1834, bytes: 18_200_000_000, problem };
    },
    library_move: async ({ to }) => {
      const from = pathsView.libraryDir ?? "D:\\PocketMods";
      const quick = String(to).slice(0, 2).toLowerCase() === from.slice(0, 2).toLowerCase();
      if (!quick) for (let k = 1; k <= 10; k++) { demoEmit("library-move", [k * 1_820_000_000, 18_200_000_000]); await new Promise((r) => setTimeout(r, 120)); }
      pathsView = { ...pathsView, libraryDir: String(to), sameVolume: String(to).slice(0, 2).toLowerCase() === (pathsView.gameDir ?? "").slice(0, 2).toLowerCase() };
      return { how: quick ? "renamed" : "copied", files: 1834, bytes: 18_200_000_000, rewritten: 2410, relinked: quick ? 0 : 912, copies: 0, leftBehind: null, copyTurnedOn: false };
    },
    game_folder_set: ({ dir }) => {
      const g = String(dir);
      const moved = !!pathsView.gameDir && pathsView.gameDir !== g;
      pathsView = { ...pathsView, gameDir: g, libraryDir: pathsView.libraryDir ?? `${g.slice(0, 2)}\\PocketMods`, defaultLibrary: `${g.slice(0, 2)}\\PocketMods`, gameLooksRight: true,
        sameVolume: (pathsView.libraryDir ?? g).slice(0, 2).toLowerCase() === g.slice(0, 2).toLowerCase(), inProgramFiles: g.toLowerCase().includes("program files") };
      return { paths: pathsView, relink: moved && installs.length ? { checked: 912, ok: 0, relinked: 912, copies: 0, restored: 0, changed: 0 } : null };
    },
    game_relink: () => ({ checked: 912, ok: 912, relinked: 0, copies: 0, restored: 0, changed: 0 }),
  });
}

// ── for mod authors: My mods from a recorded anonymous answer (src/lib/demo/my-mods.json, made by the
// creator_fixture example), plus a pretend check, pack and publish ──
const demoPins: number[] = [];
const demoMine = myModsFixture as unknown as MyMods;
const demoManage = myModFilesFixture as unknown as ModManage[];
const later = <T,>(v: T, ms = 250) => new Promise<T>((ok) => setTimeout(() => ok(v), ms));
if (!inTauri()) registerDemo({
  creator_my_mods: ({ user }) => {
    const typed = String(user ?? "").trim();
    if (!typed && !status.hasKey) throw { kind: "invalid", message: "Add your Nexus API key in Settings, or type your Nexus user name to see your mods." };
    const known = new Set(demoMine.mods.map((m) => m.modId));
    const missingPinned = demoPins.filter((id) => !known.has(id)).map((modId) => ({ modId, links: { ...demoMine.mods[0].links,
      page: `https://www.nexusmods.com/cyberpunk2077/mods/${modId}`, edit: `https://www.nexusmods.com/cyberpunk2077/mods/edit/?id=${modId}` } }));
    return later({ ...demoMine, own: status.hasKey && !typed, missingPinned, fetchedAt: Math.floor(Date.now() / 1000), fromCache: false }, 400);
  },
  creator_mod_files: ({ modId }) => {
    const d = demoManage.find((x) => x.modId === Number(modId));
    if (!d) throw { kind: "demo", message: "The demo data has no files for that mod." };
    return later(d);
  },
  creator_upload_targets: ({ modId }) => {
    if (!status.hasKey) throw { kind: "no_key", message: "No Nexus API key is set. Add one in Settings." };
    const m = demoMine.mods.find((x) => x.modId === Number(modId));
    return later((m?.files ?? []).filter((f) => f.groupId != null).map((f) => ({ id: String(f.groupId), name: f.name, versionsCount: 1 + (m?.oldFiles ?? 0),
      lastUploadedAt: new Date(f.date * 1000).toISOString(), isActive: true, groupId: f.groupId })));
  },
  creator_pin: ({ modRef }) => {
    const t = String(modRef).trim();
    const id = Number(/^\d+$/.test(t) ? t : /cyberpunk2077\/mods\/(\d+)/.exec(t)?.[1] ?? /[?&]id=(\d+)/.exec(t)?.[1] ?? NaN);
    if (!id) throw { kind: "invalid", message: "Paste the link to the mod's page (nexusmods.com/cyberpunk2077/mods/…) or its number." };
    if (!demoPins.includes(id)) demoPins.push(id);
    return id;
  },
  creator_unpin: ({ modId }) => { const k = demoPins.indexOf(Number(modId)); if (k >= 0) demoPins.splice(k, 1); },
  creator_check: ({ path, name }) => {
    const n = String(name || "My mod").replace(/[^\w -]/g, "").trim() || "mod";
    const files = [`archive/pc/mod/${n}.archive`, `archive/pc/mod/${n}.archive.xl`, `r6/scripts/${n}/${n}.reds`];
    return later({ files: files.length + 1, problems: [], tips: String(path).toLowerCase().endsWith(".zip") ? [] : ["Everything sits in one extra top folder. It works here, but packing from inside it (archive, r6 … at the top) suits every manager."],
      needs: [{ key: "archivexl", name: "ArchiveXL", modId: 4198, url: "https://www.nexusmods.com/cyberpunk2077/mods/4198" }, { key: "redscript", name: "redscript", modId: 1511, url: "https://www.nexusmods.com/cyberpunk2077/mods/1511" }],
      plan: { files: files.map((f) => ({ source: f, target: f, kind: f.endsWith(".reds") ? "script" : "archive" })), skipped: ["readme.txt"], unknown: [], modType: "mixed", warnings: [] } }, 500);
  },
  creator_package: ({ path, name, version }) => {
    const dir = String(path).replace(/[\\/][^\\/]*$/, "");
    return later(`${dir}\\${String(name).replace(/ /g, "_")}-${String(version)}.zip`, 600);
  },
  creator_publish: () => new Promise((ok) => {
    const total = 3_400_000;
    let sent = 0;
    const t = setInterval(() => {
      sent = Math.min(total, sent + 850_000);
      demoEmit("publish-progress", [sent, total]);
      if (sent >= total) { clearInterval(t); setTimeout(() => ok(0), 600); }
    }, 300);
  }),
});
