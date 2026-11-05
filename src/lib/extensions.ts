// Extensions: the installed manifests, their state, and what the turned-on ones add to the app. Screens read the
// contributions with useExtensions(). Nothing from an extension applies until the user approved it, and an update
// that needs something new waits for the user again. The format is in docs/extensions.md; the checks are in
// addon-rules.mjs (shared with the marketplace check that CI runs).
import { useEffect, useSyncExternalStore } from "react";
import { addons, extensionsApi, settings, type BrowseQuery, type ExtState, type InstallRow } from "./api";
import {
  LIMITS, approvalItems, validateExtension,
  type CommandPack, type ExtensionManifest, type GroupRule, type LaunchOption, type LinkContribution, type PanelContribution,
  type Permission, type ShelfContribution, type ShelfQuery, type ToolContribution,
} from "./addon-rules.mjs";
import { daysAgo } from "./text";

export type { ExtensionManifest, Permission } from "./addon-rules.mjs";

export interface ExtRef { extId: string; extName: string }
export interface InstalledExtension {
  id: string;
  text: string;
  manifest: ExtensionManifest | null;
  errors: string[];
  warnings: string[];
  state: ExtState;
  /** on: turned on and approved. off: turned off. review: changed since the user approved it. broken: does not check out. */
  status: "on" | "off" | "review" | "broken";
  /** What this release needs that the user has not approved. */
  unapproved: string[];
}
export interface Contributions {
  shelves: Array<ShelfContribution & ExtRef>;
  groupRules: Array<GroupRule & ExtRef>;
  /** `ready` is false for a tool whose program the user has not picked yet. */
  tools: Array<ToolContribution & ExtRef & { ready: boolean }>;
  launchOptions: Array<LaunchOption & ExtRef>;
  commandPacks: Array<CommandPack & ExtRef>;
  links: Array<LinkContribution & ExtRef>;
  panels: Array<PanelContribution & ExtRef & { release: string; permissions: Permission[] }>;
}
export interface ExtSnapshot { loaded: boolean; list: InstalledExtension[]; active: Contributions }

const none = (): Contributions => ({ shelves: [], groupRules: [], tools: [], launchOptions: [], commandPacks: [], links: [], panels: [] });
export const blankState = (): ExtState => ({ enabled: false, approvedRelease: null, approved: [], toolPaths: {}, fromMarket: false, installedAt: null });

/** Parse and check an extension file's text. */
export function checkExtensionText(text: string): { manifest: ExtensionManifest | null; errors: string[]; warnings: string[] } {
  if (text.length > LIMITS.manifest) return { manifest: null, errors: ["File is over 1 MB"], warnings: [] };
  let v: unknown;
  try { v = JSON.parse(text); } catch (e) { return { manifest: null, errors: [`Not valid JSON. ${e instanceof Error ? e.message : String(e)}`], warnings: [] }; }
  const r = validateExtension(v);
  return { manifest: r.ok ? (v as ExtensionManifest) : null, errors: r.errors, warnings: r.warnings };
}

export function readInstalled(text: string, states: Record<string, ExtState>): InstalledExtension {
  const r = checkExtensionText(text);
  let id = "unknown";
  try { const raw = JSON.parse(text) as { id?: unknown }; if (typeof raw.id === "string") id = raw.id; } catch { /* reported above */ }
  const state = states[id] ?? blankState();
  if (!r.manifest) return { id, text, manifest: null, errors: r.errors, warnings: r.warnings, state, status: "broken", unapproved: [] };
  const m = r.manifest;
  const unapproved = approvalItems(m).filter((x) => !state.approved.includes(x));
  const status = state.approvedRelease !== m.release ? "review" : state.enabled ? "on" : "off";
  return { id, text, manifest: m, errors: [], warnings: r.warnings, state, status, unapproved };
}

/** What the turned-on, approved extensions add, each item tagged with its extension. */
export function collect(list: InstalledExtension[]): Contributions {
  const out = none();
  for (const x of list) {
    if (x.status !== "on" || !x.manifest) continue;
    const m = x.manifest, k = m.contributes, ref: ExtRef = { extId: m.id, extName: m.name };
    for (const s of k.shelves ?? []) out.shelves.push({ ...s, ...ref });
    for (const r of k.groupRules ?? []) out.groupRules.push({ ...r, ...ref });
    for (const t of k.tools ?? []) out.tools.push({ ...t, ...ref, ready: !!t.path || !!x.state.toolPaths[t.id] });
    for (const o of k.launchOptions ?? []) out.launchOptions.push({ ...o, ...ref });
    for (const p of k.commandPacks ?? []) out.commandPacks.push({ ...p, ...ref });
    for (const l of k.links ?? []) out.links.push({ ...l, ...ref });
    for (const p of k.panels ?? []) out.panels.push({ ...p, ...ref, release: m.release, permissions: m.permissions ?? [] });
  }
  return out;
}

// ── the store screens read ──

let snap: ExtSnapshot = { loaded: false, list: [], active: none() };
let loading: Promise<void> | null = null;
const subs = new Set<() => void>();
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };

export function reloadExtensions(): Promise<void> {
  loading = (async () => {
    const [texts, states] = await Promise.all([addons.list("extension").catch(() => [] as string[]), extensionsApi.states().catch(() => ({} as Record<string, ExtState>))]);
    const list = texts.map((t) => readInstalled(t, states)).sort((a, b) => (a.manifest?.name ?? a.id).localeCompare(b.manifest?.name ?? b.id));
    snap = { loaded: true, list, active: collect(list) };
    subs.forEach((f) => f());
  })();
  return loading;
}

export function getExtensions(): ExtSnapshot { return snap; }

/** The installed extensions and what the turned-on ones add. Loads on first use. */
export function useExtensions(): ExtSnapshot {
  const s = useSyncExternalStore(subscribe, getExtensions);
  useEffect(() => { if (!snap.loaded && !loading) void reloadExtensions(); }, []);
  return s;
}

// ── install, approve, turn on and off, remove ──

/**
 * Save an extension. With `approve` (the user saw what it adds and can do) it is turned on for this release.
 * Without it, an update that needs nothing new keeps its approval; anything else waits for review.
 */
export async function installExtension(text: string, opts: { approve: boolean; fromMarket?: boolean; toolPaths?: Record<string, string> }): Promise<ExtensionManifest> {
  const r = checkExtensionText(text);
  if (!r.manifest) throw new Error(r.errors.slice(0, 3).join(" "));
  const m = r.manifest;
  const old = (await extensionsApi.states().catch(() => ({} as Record<string, ExtState>)))[m.id];
  const items = approvalItems(m);
  const base: ExtState = old ? { ...old } : { ...blankState(), installedAt: Math.floor(Date.now() / 1000) };
  base.toolPaths = { ...base.toolPaths, ...(opts.toolPaths ?? {}) };
  base.fromMarket = opts.fromMarket ?? base.fromMarket;
  await addons.save("extension", m.id, text);
  if (opts.approve) Object.assign(base, { enabled: true, approvedRelease: m.release, approved: items });
  else if (old && old.approvedRelease && items.every((x) => old.approved.includes(x))) Object.assign(base, { approvedRelease: m.release });
  await extensionsApi.setState(m.id, base);
  await reloadExtensions();
  return m;
}

/** What an update would need approving, given what the user approved before. */
export function newApprovals(m: ExtensionManifest, state?: ExtState | null): string[] {
  const had = state?.approved ?? [];
  return approvalItems(m).filter((x) => !had.includes(x));
}

async function patchState(id: string, f: (s: ExtState) => ExtState) {
  const all = await extensionsApi.states();
  await extensionsApi.setState(id, f(all[id] ?? blankState()));
  await reloadExtensions();
}

export async function approveExtension(id: string, toolPaths?: Record<string, string>) {
  const x = snap.list.find((e) => e.id === id);
  if (!x?.manifest) throw new Error("Extension has errors, can't turn it on");
  const m = x.manifest;
  await patchState(id, (s) => ({ ...s, enabled: true, approvedRelease: m.release, approved: approvalItems(m), toolPaths: { ...s.toolPaths, ...(toolPaths ?? {}) } }));
}
export const setExtensionEnabled = (id: string, on: boolean) => patchState(id, (s) => ({ ...s, enabled: on }));
export const setToolPath = (id: string, toolId: string, path: string) => patchState(id, (s) => ({ ...s, toolPaths: { ...s.toolPaths, [toolId]: path } }));

export async function removeExtension(id: string) {
  await addons.remove("extension", id);
  await extensionsApi.setState(id, null).catch(() => {});
  await settings.set(`ext.data.${id}`, "").catch(() => {});
  await reloadExtensions();
}

// ── contributions applied by the app ──

/** A shelf's saved search as Browse query fields. */
export function shelfQuery(q: ShelfQuery, now = Date.now()): Partial<BrowseQuery> {
  const out: Partial<BrowseQuery> = {};
  if (q.text) out.text = q.text;
  if (q.categories) out.categories = [...q.categories];
  if (q.tags) out.tags = [...q.tags];
  if (q.excludeTags) out.excludeTags = [...q.excludeTags];
  if (q.sort) out.sort = q.sort;
  if (q.ascending !== undefined) out.ascending = q.ascending;
  if (q.frameworks) out.frameworks = [...q.frameworks];
  if (q.modType) out.modType = q.modType;
  if (q.endorsementsMin !== undefined) out.endorsementsMin = q.endorsementsMin;
  if (q.downloadsMax !== undefined) out.downloadsMax = q.downloadsMax;
  if (q.createdWithinDays) out.createdAfter = daysAgo(q.createdWithinDays, now);
  if (q.sort === "random") out.seed = Math.floor(now / 86_400_000);
  return out;
}

type RuleRow = Pick<InstallRow, "modId" | "name" | "category" | "modType" | "frameworks"> & { author?: string | null };
const lc = (s: string | null | undefined) => (s ?? "").toLowerCase();

/** A mod matches a rule when any one of the rule's lists matches it. Names match by part, the rest exactly. */
export function ruleMatches(rule: GroupRule, row: RuleRow): boolean {
  const m = rule.match;
  return (m.name ?? []).some((n) => n.trim() && lc(row.name).includes(lc(n.trim())))
    || (m.category ?? []).some((c) => lc(c) === lc(row.category))
    || (m.author ?? []).some((a) => !!row.author && lc(a) === lc(row.author))
    || (m.modType ?? []).includes(row.modType)
    || (m.frameworks ?? []).some((f) => row.frameworks.includes(f))
    || (m.mods ?? []).includes(row.modId);
}

/** Where "Sort into groups" would move mods: mods in a group of your own stay put; the first matching rule wins. */
export function planGroups(rows: InstallRow[], rules: Array<GroupRule & Partial<ExtRef>>): Array<{ row: InstallRow; group: string; from: string }> {
  const out: Array<{ row: InstallRow; group: string; from: string }> = [];
  for (const row of rows) {
    if (row.customGroup) continue;
    const rule = rules.find((r) => ruleMatches(r, row));
    if (rule && rule.group !== row.group) out.push({ row, group: rule.group.trim(), from: rule.extName ?? "" });
  }
  return out;
}

/** Commands from packs that list this mod, for its detail. */
export function packCommandsFor(packs: Contributions["commandPacks"], modId: number): Array<{ command: string; generated: false; from: string; label?: string }> {
  return packs.filter((p) => p.mods?.includes(modId)).flatMap((p) => p.commands.map((c) => ({ command: c.command, generated: false as const, from: p.title, label: c.label })));
}

/** Plain words for an approval item, for the install and review dialogs. */
export function describeItem(item: string, m: ExtensionManifest): string {
  const [kind, id, ...rest] = item.split(":");
  const k = m.contributes;
  if (kind === "permission") return id;
  if (kind === "tool") { const t = k.tools?.find((x) => x.id === id); return t ? `Start ${t.name}${t.path ? ` (${t.path} in the game folder)` : " (a program you pick)"}` : item; }
  if (kind === "launch") { const o = k.launchOptions?.find((x) => x.id === id); return o ? `Start the game with ${o.args.join(" ")}` : item; }
  if (kind === "link") return `Open ${rest.length ? `${id}:${rest.join(":")}` : id}`;
  return item;
}
