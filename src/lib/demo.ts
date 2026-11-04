// Demo backend for running the UI in a plain browser (npm run dev). It answers the same commands as the Rust side
// with fixed data recorded from the live Nexus API (src/lib/demo/*.json, made by the demo_fixture example), so
// screens can be built and checked without Tauri. Nothing here talks to Nexus.
import { demoEmit, inTauri, registerDemo } from "./tauri";
import type { BrowseQuery, BrowseResult, CardView, DetailView, LocalFilter, NexusStatus, RateDisplay } from "./api";
import browseFixture from "./demo/browse.json";
import detailFixture from "./demo/details.json";

const rate: RateDisplay = { level: "unknown", remaining: null, limit: null, window: "hour", resetsAt: null, throttledUntil: null };
let status: NexusStatus = { hasKey: false, user: null, rateLimit: rate };
const flags = new Map<number, { hidden: boolean; ignored: boolean }>();
const store = new Map<string, string>();
const fixture = browseFixture as unknown as BrowseResult;
const details = detailFixture as unknown as DetailView[];

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
    patches: () => [["2.0", "2023-09-21"], ["2.1", "2023-12-05"], ["2.12", "2024-03-01"], ["2.2", "2024-12-10"], ["2.3", "2025-07-17"], ["2.5", "2026-10-22"]]
      .map(([version, released]) => [{ version, released }, Date.parse(`${released}T00:00:00Z`) / 1000]),
  });
}
