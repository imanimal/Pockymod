// The app's side of a panel's postMessage API. A panel runs in a sandboxed frame with an opaque origin and no
// network; this is its only way to reach the app. Each method needs a permission the extension declared and the
// user approved, arguments are checked, and calls are rate limited. See docs/extensions.md for the panel side.
import { browse, bulk, library, nexusPages, settings } from "./api";
import { LIMITS, type Permission } from "./addon-rules.mjs";

export interface PanelInfo { extId: string; extName: string; release: string; panelId: string; title: string; permissions: Permission[] }
/** The parts of the app a bridge needs from the screen it is in. */
export interface PanelHostUi {
  /** Ask the user to queue a download. Resolves true when they say yes; "block" stops the panel asking again. */
  confirmQueue: (modId: number, fileId: number | null) => Promise<boolean | "block">;
  toast: (text: string, bad?: boolean) => void;
}

/** The app's colours, sent to a panel so it can match. */
export const THEME_TOKENS = ["--bg", "--bg2", "--surface", "--surface-2", "--line", "--text", "--text-2", "--text-3", "--accent", "--on-accent",
  "--chrome-bg", "--chrome-fg", "--disp-bg", "--disp-fg", "--ok", "--warn", "--bad", "--acc2"];
export function panelTheme(): { scheme: string; tokens: Record<string, string> } {
  const root = document.documentElement;
  const cs = getComputedStyle(root);
  const tokens: Record<string, string> = {};
  for (const k of THEME_TOKENS) { const v = cs.getPropertyValue(k).trim(); if (v) tokens[k] = v; }
  return { scheme: root.dataset.scheme === "dark" ? "dark" : "light", tokens };
}

export class PanelError extends Error {}
/** The message for calls over the rate limit: the host drops these without an answer. */
export const TOO_MANY = "Too many calls. Slow down.";
const fail = (msg: string): never => { throw new PanelError(msg); };

/** Calls per window, per method group. */
const LIMIT: Record<string, [number, number]> = { any: [40, 1000], "nexus.search": [30, 60_000], toast: [3, 5000], "queue.add": [1, 0], "library.setEnabled": [20, 10_000] };

const int = (v: unknown, name: string) => (Number.isInteger(v) && (v as number) > 0 ? (v as number) : fail(`${name} must be a whole number.`));
const text = (v: unknown, name: string, max: number) => (typeof v === "string" && v.length <= max ? v : fail(`${name} must be text of up to ${max} characters.`));

const PERMISSION_OF: Record<string, Permission | null> = {
  hello: null, "library.list": "library.read", "library.setEnabled": "library.toggle", "nexus.search": "nexus.search", "nexus.open": "nexus.open",
  "queue.add": "queue.add", toast: "toast", "storage.get": "storage", "storage.set": "storage", "storage.remove": "storage", "storage.keys": "storage",
};

export function makeBridge(info: PanelInfo, ui: PanelHostUi) {
  const stamps = new Map<string, number[]>();
  let confirming = false;
  // a "no" holds for a minute; three of them, or "Block", hold until the app restarts
  let refusedAt = 0, refusals = 0, blocked = false;
  const allow = (group: string) => {
    const [max, per] = LIMIT[group] ?? LIMIT.any;
    if (per === 0) return;
    const now = Date.now();
    const list = (stamps.get(group) ?? []).filter((t) => now - t < per);
    if (list.length >= max) fail(TOO_MANY);
    list.push(now);
    stamps.set(group, list);
  };
  const storeKey = `ext.data.${info.extId}`;
  const readStore = async (): Promise<Record<string, unknown>> => {
    const raw = await settings.get(storeKey).catch(() => null);
    if (!raw) return {};
    try { const v = JSON.parse(raw); return v && typeof v === "object" && !Array.isArray(v) ? v : {}; } catch { return {}; }
  };

  return async function handle(method: string, params: unknown): Promise<unknown> {
    if (!(method in PERMISSION_OF)) fail(`There is no method called ${method}.`);
    const need = PERMISSION_OF[method];
    if (need && !info.permissions.includes(need)) fail(`This extension did not ask for the ${need} permission.`);
    allow("any");
    if (LIMIT[method]) allow(method);
    const p = (params && typeof params === "object" ? params : {}) as Record<string, unknown>;

    switch (method) {
      case "hello":
        return { extension: { id: info.extId, name: info.extName, release: info.release }, panel: { id: info.panelId, title: info.title }, permissions: info.permissions, theme: panelTheme() };
      case "library.list": {
        const [rows, downloads] = await Promise.all([library.list(), library.downloads().catch(() => [])]);
        return rows.map((r) => ({
          id: r.id, modId: r.modId < 900_000_000 ? r.modId : null, name: r.name, version: r.version, enabled: r.enabled, group: r.group,
          category: r.category, modType: r.modType, frameworks: r.frameworks, files: r.deployedFiles, source: r.source,
          size: downloads.find((d) => d.modId === r.modId && d.fileId === r.fileId)?.size ?? null,
        }));
      }
      case "library.setEnabled": {
        const id = int(p.id, "id");
        if (typeof p.on !== "boolean") fail("on must be true or false.");
        const row = (await library.list()).find((r) => r.id === id) ?? fail("There is no mod with that id in your library.");
        const r = await bulk([id], p.on ? "enable" : "disable");
        if (r.failed.length) return { ok: false, reason: r.failed[0][1] };
        ui.toast(`${info.extName} turned ${row.name} ${p.on ? "on" : "off"}.`);
        return { ok: true };
      }
      case "nexus.search": {
        const count = p.count === undefined ? 12 : Math.min(30, int(p.count, "count"));
        const sorts = ["downloads", "endorsements", "updated", "created", "relevance"];
        const sort = p.sort === undefined ? (p.text ? "relevance" : "endorsements") : sorts.includes(String(p.sort)) ? String(p.sort) : fail(`sort must be one of ${sorts.join(", ")}.`);
        const r = await browse.search({
          text: p.text === undefined ? "" : text(p.text, "text", 100), categories: [], tags: [], excludeTags: [], sort: sort as never, ascending: false,
          includeAdult: false, frameworks: [], modType: null, offset: 0, count,
        }, { showHidden: false, downloaded: null, skipCounts: true });
        return {
          total: r.total,
          mods: r.nodes.map((n) => ({ modId: n.modId, name: n.name, summary: n.summary, author: n.author ?? n.uploader?.name ?? null, category: n.category, version: n.version,
            downloads: n.downloads, endorsements: n.endorsements, updatedAt: n.updatedAt, modType: n.classification.modType, downloaded: n.downloaded })),
        };
      }
      case "nexus.open":
        await nexusPages.mod(int(p.modId, "modId"));
        return null;
      case "queue.add": {
        const modId = int(p.modId, "modId");
        const fileId = p.fileId === undefined || p.fileId === null ? null : int(p.fileId, "fileId");
        if (blocked) fail("The user blocked this panel's download requests.");
        if (Date.now() - refusedAt < 60_000) fail("The user said no a moment ago. Ask again later.");
        if (confirming) fail("The app is already asking about a download.");
        confirming = true;
        try {
          const answer = await ui.confirmQueue(modId, fileId);
          if (answer !== true) { refusedAt = Date.now(); refusals++; if (answer === "block" || refusals >= 3) blocked = true; }
          return { queued: answer === true };
        } finally { confirming = false; }
      }
      case "toast":
        ui.toast(`${info.extName}: ${text(p.text, "text", 200)}`);
        return null;
      case "storage.get": { const key = text(p.key, "key", 100); const all = await readStore(); return key in all ? all[key] : null; }
      case "storage.keys": return Object.keys(await readStore());
      case "storage.set":
      case "storage.remove": {
        const key = text(p.key, "key", 100);
        const all = await readStore();
        if (method === "storage.remove") delete all[key];
        else {
          if (p.value === undefined) fail("value is missing. Use storage.remove to delete a key.");
          all[key] = JSON.parse(JSON.stringify(p.value));
        }
        const out = JSON.stringify(all);
        if (out.length > LIMITS.storage) fail("The extension's storage is full (256 KB).");
        await settings.set(storeKey, out);
        return null;
      }
    }
    return fail(`There is no method called ${method}.`);
  };
}

/** A message from a panel, if it is one: `{ pocket: 1, id, method, params }`. */
export function readCall(data: unknown): { id: number; method: string; params: unknown } | null {
  if (!data || typeof data !== "object") return null;
  const m = data as Record<string, unknown>;
  if (m.pocket !== 1 || !Number.isInteger(m.id) || typeof m.method !== "string" || m.method.length > 40) return null;
  return { id: m.id as number, method: m.method, params: m.params ?? null };
}
