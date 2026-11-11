// Browse, built for discovery. With nothing typed it is a front page of shelves (popular now, updated for your
// patch, surprise me, hidden gems …) and category tiles; typing or filtering turns it into a grid. Every card has a
// key: Quick install, or for a mod you have, the menu of what the app can do to it. A mod opens in a drawer whose
// Install key is the screen's one orange key, or expanded as a page like the mod's Nexus page.
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "../ui/Icon";
import { ICONS, type IconName } from "../ui/icons";
import { Chip, Empty, Key, Module, Pill, SearchField, Seg, Switch, formatCount } from "../ui/pocket";
import {
  FRAMEWORKS, MOD_TYPES, browse, errorText, game, settings,
  type BrowseQuery, type BrowseResult, type CardView, type DetailView, type InstallRow, type LocalFilter, type ModTypeFilter, type SortKey, type Update,
} from "../lib/api";
import { daysAgo, shortDate } from "../lib/text";
import { Detail, FrameworkTags, TypeTag } from "./browse/Detail";
import { shelfQuery, useExtensions } from "../lib/extensions";
import { Modio } from "./Modio";
import { MediaWall } from "./browse/Media";
import { ModKey, useInstalled } from "./browse/ModActions";
import { ADULT_OPTIONS, setAdultPref, setAiPref, useAdult, useAiHide, type Adult } from "../lib/prefs";
import { AI_OPTIONS, aiHideText, withAiHidden, type AiHide } from "../lib/ai";
import { AiTags } from "../ui/AiTags";
import { toast } from "../ui/toast";

/** What the cards need to show their key: the user's copies of each mod, the queue, updates and the flag setter. */
interface CardCtx { byMod: Map<number, InstallRow[]>; queued: Map<number, string>; updates: Update[]; onLibrary?: () => void; onFlag: (c: CardView, f: "hidden" | "ignored", on: boolean) => void }
const Cards = createContext<CardCtx | null>(null);
const pageKey = "pm.modPage";

const PAGE = 30;
const SORTS: Array<{ value: SortKey; label: string }> = [
  { value: "relevance", label: "Best match" }, { value: "endorsements", label: "Most endorsed" }, { value: "downloads", label: "Most downloaded" },
  { value: "updated", label: "Recently updated" }, { value: "created", label: "Newest" }, { value: "size", label: "Largest" }, { value: "random", label: "Shuffle" },
];
type Since = "any" | "7" | "30" | "365";
type Dl = "any" | "1000" | "10000" | "100000";
type Size = "any" | "10" | "100" | "1024";
type PatchOpt = { version: string; at: number; yours: boolean };

export const emptyQuery = (): BrowseQuery => ({
  text: "", categories: [], tags: [], excludeTags: [], sort: "endorsements", ascending: false, includeAdult: false, frameworks: [], modType: null, offset: 0, count: PAGE,
});

const daySeed = () => Math.floor(Date.now() / 86_400_000);
/** Nexus's trending window for Cyberpunk 2077 (`Game.trendingPeriodDays`, 7 when checked live). */
const TRENDING_DAYS = 7;

/** Last calendar month as Unix seconds, for "Best of October". Nexus has no Mod of the Month field. */
function lastMonth(now = new Date()) {
  const start = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() - 1, 1));
  const end = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 1));
  return { name: start.toLocaleDateString("en-GB", { month: "long", timeZone: "UTC" }), from: start.getTime() / 1000, to: end.getTime() / 1000 - 1 };
}

type View = "discover" | "trending" | "new" | "updated" | "popular" | "surprise" | "media";
const VIEWS: Array<{ value: View; label: string }> = [
  { value: "discover", label: "Discover" }, { value: "trending", label: "Trending" }, { value: "new", label: "New" },
  { value: "updated", label: "Updated" }, { value: "popular", label: "Popular" }, { value: "surprise", label: "Surprise" }, { value: "media", label: "Media" },
];
const VIEW_SORT: Partial<Record<View, SortKey>> = { trending: "endorsements", new: "created", updated: "updated", popular: "endorsements", surprise: "random" };

/** The front page's shelves: one cached `mods()` page each, no extra counts. */
function shelves(patch: PatchOpt | null, seed: number): Array<{ id: string; title: string; icon: IconName; hint: string; q: Partial<BrowseQuery> }> {
  return [
    { id: "trending", title: "Trending", icon: "up", hint: `New in the last ${TRENDING_DAYS} days, most endorsed`, q: { createdAfter: daysAgo(TRENDING_DAYS), sort: "endorsements" } },
    ...(patch ? [{ id: "patch", title: `Updated for ${patch.version}`, icon: "check" as IconName, hint: `Updated since patch ${patch.version}${patch.yours ? ", your game" : ""}`, q: { compatibleSince: patch.at, sort: "endorsements" as SortKey } }] : []),
    { id: "surprise", title: "Surprise me", icon: "refresh", hint: "Random, 100+ endorsements", q: { sort: "random", seed, endorsementsMin: 100 } },
    { id: "month", title: `Best of ${lastMonth().name}`, icon: "star", hint: `Released in ${lastMonth().name}, most endorsed`, q: { createdAfter: lastMonth().from, createdBefore: lastMonth().to, sort: "endorsements" } },
    { id: "gems", title: "Hidden gems", icon: "eye", hint: "Well endorsed, under 25k downloads", q: { endorsementsMin: 150, downloadsMax: 25_000, sort: "endorsements" } },
    { id: "classics", title: "All-time favourites", icon: "library", hint: "Most endorsed ever", q: { sort: "endorsements" } },
    { id: "new", title: "Just released", icon: "clock", hint: "Newest uploads", q: { sort: "created" } },
    { id: "frameworks", title: "Frameworks and essentials", icon: "chip", hint: "What other mods need", q: { modType: "framework", sort: "downloads" } },
  ];
}

export function Browse({ onInstall, onOpenPage, updates, onLibrary }: {
  onInstall?: (d: DetailView, fileId: number) => void; onOpenPage?: (modId: number) => void; updates?: Update[]; onLibrary?: () => void;
}) {
  const [q, setQ] = useState<BrowseQuery>(emptyQuery);
  // the search and the open mod come back with the window (local storage)
  const [text, setText] = useState(() => { try { return localStorage.getItem("pm.session.browse.text") ?? ""; } catch { return ""; } });
  useEffect(() => { try { localStorage.setItem("pm.session.browse.text", text); } catch { /* storage unavailable */ } }, [text]);
  const [local, setLocal] = useState<LocalFilter>({ showHidden: false, downloaded: null });
  const [since, setSince] = useState<Since>("any");
  const [dl, setDl] = useState<Dl>("any");
  const [size, setSize] = useState<Size>("any");
  const [patches, setPatches] = useState<PatchOpt[]>([]);
  const [patchSel, setPatchSel] = useState<string>("auto");
  const [results, setResults] = useState<CardView[]>([]);
  const [res, setRes] = useState<BrowseResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [sel, setSel] = useState<CardView | null>(() => { try { return JSON.parse(localStorage.getItem("pm.session.browse.sel") ?? "null"); } catch { return null; } });
  useEffect(() => { try { localStorage.setItem("pm.session.browse.sel", JSON.stringify(sel)); } catch { /* storage unavailable */ } }, [sel]);
  const [drawer, setDrawer] = useState(false);
  const [moreTags, setMoreTags] = useState(false);
  const [explore, setExplore] = useState(false);
  const [source, setSource] = useState<"nexus" | "modio">("nexus");
  const [modioOn, setModioOn] = useState(false);
  const [seed, setSeed] = useState(daySeed);
  const [view, setView] = useState<View>("discover");
  const inst = useInstalled();
  // a mod opens in the drawer, or as a page (the last choice is kept); a page remembers the mods it came through
  const [wide, setWideState] = useState(() => { try { return localStorage.getItem(pageKey) === "page"; } catch { return false; } });
  const [trail, setTrail] = useState<CardView[]>([]);
  const listTop = useRef(0);
  const viewEl = () => document.querySelector<HTMLElement>(".shell > .view");
  const setWide = (w: boolean) => {
    setWideState(w);
    try { localStorage.setItem(pageKey, w ? "page" : "drawer"); } catch { /* storage unavailable */ }
    if (w) { listTop.current = viewEl()?.scrollTop ?? 0; requestAnimationFrame(() => viewEl()?.scrollTo({ top: 0 })); }
    else requestAnimationFrame(() => viewEl()?.scrollTo({ top: listTop.current }));
  };
  const open = (c: CardView | null) => {
    if (c && wide && !sel) { listTop.current = viewEl()?.scrollTop ?? 0; requestAnimationFrame(() => viewEl()?.scrollTo({ top: 0 })); }
    if (!c && wide && sel) requestAnimationFrame(() => viewEl()?.scrollTo({ top: listTop.current }));
    setTrail([]);
    setSel(c);
  };
  const pickView = (v: View) => { setView(v); open(null); if (VIEW_SORT[v]) set({ sort: VIEW_SORT[v]! }); if (v === "discover") clearAll(); };

  useEffect(() => {
    const read = () => settings.get("flags.modio").then((v) => setModioOn(v === "1")).catch(() => {});
    read();
    window.addEventListener("pm-flags", read);
    return () => window.removeEventListener("pm-flags", read);
  }, []);

  // patches for the dropdown, your game's first: read from the game, else the Settings choice, else the newest
  useEffect(() => {
    (async () => {
      const list = await browse.patches();
      const info = await game.inspect().catch(() => null);
      const saved = await settings.get("game.patch").catch(() => null);
      const mine = info?.detectedPatch ?? (saved || null) ?? list[list.length - 1]?.[0].version;
      const opts: PatchOpt[] = list.filter(([, at]) => at).map(([p, at]) => ({ version: p.version, at: at!, yours: p.version === mine })).reverse();
      opts.sort((a, b) => Number(b.yours) - Number(a.yours));
      setPatches(opts);
    })().catch(() => {});
  }, []);
  const yours = patches.find((p) => p.yours) ?? null;
  const patch = patchSel === "auto" ? yours : patchSel === "any" ? null : patches.find((p) => p.version === patchSel) ?? null;

  useEffect(() => {
    const t = setTimeout(() => setQ((old) => (old.text ?? "") === text ? old : { ...old, text, offset: 0, sort: text && old.sort === "endorsements" ? "relevance" : !text && old.sort === "relevance" ? "endorsements" : old.sort }), 350);
    return () => clearTimeout(t);
  }, [text]);

  const adult = useAdult();
  const aiHide = useAiHide();
  const effective = useMemo<BrowseQuery>(() => withAiHidden({
    ...q,
    includeAdult: adult !== "hide",
    seed: q.sort === "random" ? seed : null,
    updatedAfter: since === "any" ? null : daysAgo(Number(since)),
    downloadsMin: dl === "any" ? null : Number(dl),
    sizeMaxKb: size === "any" ? null : Number(size) * 1024,
    compatibleSince: patch?.at ?? null,
    createdAfter: q.createdAfter ?? (view === "trending" ? daysAgo(TRENDING_DAYS) : null),
  }, aiHide), [q, adult, aiHide, since, dl, size, patch, seed, view]);

  const filtering = !!(q.text || q.categories.length || q.tags.length || q.excludeTags.length || q.frameworks.length || q.modType
    || since !== "any" || dl !== "any" || size !== "any" || local.downloaded !== null || local.showHidden || explore || view !== "discover");

  const seq = useRef(0);
  const lastKey = useRef("");
  // what the list is, apart from its offset; built from the choices, not `effective`, whose dates move with the clock
  const listKey = JSON.stringify([{ ...q, offset: 0 }, adult !== "hide", aiHide, since, dl, size, patch?.at ?? null, q.sort === "random" ? seed : null, view, local]);
  // more: this search adds the next page under the list (Show more) rather than starting it again
  const [more, setMore] = useState(false);
  useEffect(() => {
    const n = ++seq.current;
    // a change to anything but the offset (a filter, the patch, the adult setting) starts the list from the top:
    // the offset goes back to 0 and this runs again
    const next = effective.offset > 0 && listKey === lastKey.current;
    lastKey.current = listKey;
    if (effective.offset > 0 && !next) { setQ((old) => ({ ...old, offset: 0 })); return; }
    setBusy(true); setMore(next); setErr(null);
    browse.search(effective, local)
      .then((r) => { if (n !== seq.current) return; setRes(r); setResults((old) => next ? [...old, ...r.nodes.filter((x) => !old.some((o) => o.modId === x.modId))] : r.nodes); })
      .catch((e) => { if (n === seq.current) setErr(errorText(e)); })
      .finally(() => { if (n === seq.current) setBusy(false); });
  }, [effective, local, listKey]);
  // the same query again (the same page, if Show more failed)
  const retry = () => setQ((old) => ({ ...old }));

  const set = (patchQ: Partial<BrowseQuery>) => setQ((old) => ({ ...old, ...patchQ, offset: patchQ.offset ?? 0 }));
  const toggle = <T,>(list: T[], v: T) => (list.includes(v) ? list.filter((x) => x !== v) : [...list, v]);
  const derived = useMemo(() => new Map(res?.derived ?? []), [res]);
  const clearAll = () => { setQ({ ...emptyQuery(), text }); setSince("any"); setDl("any"); setSize("any"); setLocal({ showHidden: false, downloaded: null }); setExplore(false); setView("discover"); };

  const flag = useCallback(async (c: CardView, which: "hidden" | "ignored", on: boolean) => {
    try { await browse.setFlag(c.modId, which, on); } catch (e) { toast(errorText(e), true); return false; }
    setResults((r) => r.map((n) => n.modId === c.modId ? { ...n, [which]: on } : n).filter((n) => local.showHidden || !n.hidden));
    if (which === "hidden" && on && !wide) setSel(null);
    return true;
  }, [local.showHidden, wide]);
  const cards = useMemo<CardCtx>(() => ({ byMod: inst.byMod, queued: inst.queued, updates: updates ?? [], onLibrary, onFlag: (c, f, on) => void flag(c, f, on) }), [inst.byMod, inst.queued, updates, onLibrary, flag]);

  // a shelf's own patch is the one the front page showed it for (yours when the dropdown says any)
  const openShelf = (sq: Partial<BrowseQuery>) => { set({ ...sq }); setExplore(true); if (sq.compatibleSince === undefined) setPatchSel("any"); else if (!patch) setPatchSel("auto"); };

  // active filters as removable chips
  const active: Array<{ label: string; off: () => void }> = [
    ...q.frameworks.map((f) => ({ label: `Needs ${FRAMEWORKS.find((x) => x.key === f)?.short}`, off: () => set({ frameworks: q.frameworks.filter((x) => x !== f) }) })),
    ...(q.modType ? [{ label: MOD_TYPES.find((t) => t.key === q.modType)?.name ?? "", off: () => set({ modType: null }) }] : []),
    ...q.categories.map((c) => ({ label: c, off: () => set({ categories: q.categories.filter((x) => x !== c) }) })),
    ...q.tags.map((t) => ({ label: `#${t}`, off: () => set({ tags: q.tags.filter((x) => x !== t) }) })),
    ...q.excludeTags.map((t) => ({ label: `not #${t}`, off: () => set({ excludeTags: q.excludeTags.filter((x) => x !== t) }) })),
    ...(aiHide !== "show" ? [{ label: aiHideText(aiHide), off: () => void setAiPref("show").catch((e) => toast(errorText(e), true)) }] : []),
    ...(since !== "any" ? [{ label: `Updated in the last ${since === "7" ? "week" : since === "30" ? "month" : "year"}`, off: () => setSince("any") }] : []),
    ...(dl !== "any" ? [{ label: `${Number(dl) / 1000}k+ downloads`, off: () => setDl("any") }] : []),
    ...(size !== "any" ? [{ label: `Under ${size === "1024" ? "1 GB" : `${size} MB`}`, off: () => setSize("any") }] : []),
    ...(local.downloaded !== null ? [{ label: local.downloaded ? "Downloaded" : "Not downloaded", off: () => setLocal({ ...local, downloaded: null }) }] : []),
    ...(local.showHidden ? [{ label: "Showing hidden", off: () => setLocal({ ...local, showHidden: false }) }] : []),
    ...(q.text && !text ? [{ label: `"${q.text}"`, off: () => set({ text: "" }) }] : []),
  ];

  if (modioOn && source === "modio") return (
    <div className="stack">
      <Seg label="Source" value={source} onChange={setSource} options={[{ value: "nexus", label: "Nexus Mods" }, { value: "modio", label: "mod.io (read-only)" }]} />
      <Modio />
    </div>
  );

  const detail = sel && (
    <Detail card={sel} onFlag={flag} onInstall={onInstall} onOpenPage={onOpenPage} wide={wide} onWide={setWide}
      onClose={() => { if (wide && trail.length) { setSel(trail[trail.length - 1]); setTrail(trail.slice(0, -1)); } else open(null); }}
      installed={inst.byMod.get(sel.modId) ?? []} updates={updates} queued={inst.queued.get(sel.modId)} onLibrary={onLibrary}
      onOpen={(id) => { if (wide) setTrail([...trail, sel]); setSel({ ...sel, modId: id, name: "", updatedAt: "" } as CardView); }} />
  );
  const paged = !!sel && wide;
  // Back (Alt+Left, a controller's B) leaves a page; Escape closes the drawer
  useEffect(() => {
    const back = () => { if (!sel) return; if (wide && trail.length) { setSel(trail[trail.length - 1]); setTrail(trail.slice(0, -1)); } else open(null); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape" && sel && !document.querySelector(".lightbox, .ov, .pmenu.popover")) back(); };
    window.addEventListener("pm-back", back);
    window.addEventListener("keydown", esc);
    return () => { window.removeEventListener("pm-back", back); window.removeEventListener("keydown", esc); };
  });
  return (
    <Cards.Provider value={cards}>
    {paged && <div className="mpage-wrap">
      <div className="actions">
        <Key size="sm" variant="ghost" icon="chevron" className="flip" onClick={() => { if (trail.length) { setSel(trail[trail.length - 1]); setTrail(trail.slice(0, -1)); } else open(null); }}>
          {trail.length ? `Back to ${trail[trail.length - 1].name || "the last mod"}` : "Back to the list"}</Key>
      </div>
      {detail}
    </div>}
    <div className="disc" style={paged ? { display: "none" } : undefined}>
      <div className="browse-top">
        {modioOn && <Seg label="Source" value={source} onChange={setSource} options={[{ value: "nexus", label: "Nexus Mods" }, { value: "modio", label: "mod.io" }]} />}
        {filtering && <Key size="sm" variant="ghost" icon="browse" onClick={clearAll} title="Back to the front page">Front page</Key>}
        <SearchField value={text} onChange={setText} placeholder="Search Cyberpunk 2077 mods" />
        <label className="patchsel" title="Show mods updated since this game patch">
          <Icon name="play" size={9} />
          <select value={patchSel} onChange={(e) => setPatchSel(e.target.value)} aria-label="Game version">
            <option value="auto">{yours ? `Updated since ${yours.version} (your game)` : "Your game version"}</option>
            {patches.filter((p) => !p.yours).map((p) => <option key={p.version} value={p.version}>Updated since {p.version}</option>)}
            <option value="any">Any game version</option>
          </select>
        </label>
        <Key size="sm" icon="filter" latched={drawer} onClick={() => setDrawer(!drawer)}>Filters{active.length ? ` · ${active.length}` : ""}</Key>
      </div>

      <div className="vtabs" role="tablist" aria-label="Views">
        {VIEWS.map((v) => <Chip key={v.value} on={view === v.value} onClick={() => pickView(v.value)}>{v.label}</Chip>)}
      </div>

      {view === "media" ? <MediaWall /> : <div className={`disc-body${drawer ? " with-drawer" : ""}`}>
        {drawer && (
          <Module icon="filter" title="Filters" count={active.length ? `${active.length} on` : undefined} className="filters"
            tools={<Key size="sm" variant="ghost" icon="x" onClick={() => setDrawer(false)} aria-label="Close filters" title="Close filters" />}>
            <div className="sub-h">Needs framework</div>
            <div className="chips">
              {FRAMEWORKS.map((f) => <Chip key={f.key} on={q.frameworks.includes(f.key)} count={derived.get(`fw_${f.key}`)} onClick={() => { set({ frameworks: toggle(q.frameworks, f.key) }); setExplore(true); }} title={f.name}>{f.short}</Chip>)}
            </div>
            <div className="sub-h">Mod type</div>
            <div className="chips">
              {MOD_TYPES.map((t) => <Chip key={t.key} on={q.modType === t.key} count={derived.get(`type_${t.key}`)} onClick={() => { set({ modType: q.modType === t.key ? null : t.key as ModTypeFilter }); setExplore(true); }}>{t.name}</Chip>)}
            </div>
            <div className="sub-h">Updated</div>
            <Seg label="Updated" value={since} onChange={setSince} options={[{ value: "any", label: "Any" }, { value: "7", label: "Week" }, { value: "30", label: "Month" }, { value: "365", label: "Year" }]} />
            <div className="sub-h">Downloads</div>
            <Seg label="Downloads" value={dl} onChange={setDl} options={[{ value: "any", label: "Any" }, { value: "1000", label: "1k+" }, { value: "10000", label: "10k+" }, { value: "100000", label: "100k+" }]} />
            <div className="sub-h">Size</div>
            <Seg label="Size" value={size} onChange={setSize} options={[{ value: "any", label: "Any" }, { value: "10", label: "<10 MB" }, { value: "100", label: "<100 MB" }, { value: "1024", label: "<1 GB" }]} />
            <div className="sub-h">Tags <span className="dim">· shift-click to exclude</span></div>
            <div className="chips">
              {(res?.facets.tags ?? []).filter(([t]) => !q.excludeTags.includes(t)).slice(0, moreTags ? 120 : 16).map(([name, n]) => (
                <button key={name} type="button" className={`chip${q.tags.includes(name) ? " on" : ""}`} aria-pressed={q.tags.includes(name)}
                  onClick={(e) => { setExplore(true); e.shiftKey ? set({ excludeTags: toggle(q.excludeTags, name), tags: q.tags.filter((t) => t !== name) }) : set({ tags: toggle(q.tags, name) }); }}>
                  {name}<span className="n">{formatCount(n)}</span>
                </button>
              ))}
              {(res?.facets.tags.length ?? 0) > 16 && <Key size="sm" variant="ghost" onClick={() => setMoreTags(!moreTags)}>{moreTags ? "Fewer" : "More"}</Key>}
            </div>
            <div className="sub-h">On this PC</div>
            <Seg label="Downloaded" value={local.downloaded === null ? "all" : local.downloaded ? "yes" : "no"}
              onChange={(v) => { setLocal({ ...local, downloaded: v === "all" ? null : v === "yes" }); setExplore(true); }}
              options={[{ value: "all", label: "All" }, { value: "yes", label: "Downloaded" }, { value: "no", label: "Not yet" }]} />
            <div className="stack" style={{ gap: ".5em", marginTop: ".6em" }}>
              <Switch checked={local.showHidden} onChange={(v) => setLocal({ ...local, showHidden: v })}>Show mods I hid</Switch>
            </div>
            <div className="sub-h">Adult</div>
            <Seg<Adult> label="Adult content" value={adult} onChange={(x) => void setAdultPref(x).catch((e) => toast(errorText(e), true))} options={ADULT_OPTIONS} />
            <div className="sub-h">AI <span className="dim">· by Nexus's AI tags</span></div>
            <Seg<AiHide> label="AI-tagged mods" value={aiHide} onChange={(x) => void setAiPref(x).catch((e) => toast(errorText(e), true))} options={AI_OPTIONS} />
          </Module>
        )}

        <div className="disc-main">
          {err && <div className="note bad">{err} <button type="button" className="linkish" onClick={retry}>Try again</button></div>}
          {!filtering ? (
            <FrontPage patch={patch ?? yours} seed={seed} onReshuffle={() => setSeed((s) => s + 1)} onOpen={open} onSeeAll={openShelf}
              categories={res?.facets.categories ?? []} loading={busy && !res} onCategory={(c) => { set({ categories: [c] }); setExplore(true); }} includeAdult={adult !== "hide"} />
          ) : (
            <>
              <div className="results-bar">
                <b className="hi">{res && !(busy && !more) ? `${formatCount(res.total)} ${res.total === 1 ? "mod" : "mods"}` : "Searching"}</b>
                {res?.fromCache && <span className="dim">· from the cache</span>}
                {active.map((a) => <button key={a.label} type="button" className="chip on" onClick={a.off}>{a.label}<span className="x">×</span></button>)}
                {active.length > 1 && <Key size="sm" variant="ghost" onClick={clearAll}>Clear all</Key>}
                <span className="sp" />
                <select value={q.sort} onChange={(e) => set({ sort: e.target.value as SortKey })} aria-label="Sort">
                  {SORTS.filter((s) => s.value !== "relevance" || q.text).map((s) => <option key={s.value} value={s.value}>{s.label}</option>)}
                </select>
                {q.sort === "random" && <Key size="sm" icon="refresh" onClick={() => setSeed((s) => s + 1)}>Shuffle again</Key>}
              </div>
              {!busy && !err && results.length === 0 && <Empty icon="browse" title="No mods match">Try fewer filters or another game version</Empty>}
              {(view === "trending" || view === "popular") && active.length === 0 && results.length > 2 && (
                <div className="hero2">{results.slice(0, 2).map((c) => <HeroCard key={c.modId} c={c} onOpen={() => open(c)} />)}</div>
              )}
              <div className="mgrid">
                {results.slice((view === "trending" || view === "popular") && active.length === 0 && results.length > 2 ? 2 : 0).map((c) => <ModCard key={c.modId} c={c} onOpen={() => open(c)} />)}
                {busy && results.length === 0 && Array.from({ length: 8 }, (_, i) => <span key={`skel${i}`} className="mcard panel skel" aria-hidden="true" />)}
              </div>
              {res && results.length < res.total && (
                <div className="actions" style={{ justifyContent: "center", margin: "1em 0" }}>
                  <Key icon="down" busy={busy && more} disabled={busy} onClick={() => set({ offset: results.length })}>Show more ({formatCount(res.total - results.length)} left)</Key>
                </div>
              )}
            </>
          )}
        </div>
      </div>}

      {sel && !wide && (
        <div className="drawer-ov" onMouseDown={(e) => { if (e.target === e.currentTarget) open(null); }}>
          <div className="drawer" role="dialog" aria-label={sel.name || "Mod"}>{detail}</div>
        </div>
      )}
    </div>
    </Cards.Provider>
  );
}

/** A mod as a card: a small module whose band carries the category and downloads. The whole card opens the mod;
 *  the key on its picture installs it, or opens the menu for a mod you have. */
export function ModCard({ c, onOpen, compact }: { c: CardView; onOpen: () => void; compact?: boolean }) {
  const ctx = useContext(Cards);
  const rows = ctx?.byMod.get(c.modId) ?? [];
  const veil = useAdult() === "blur" && !!c.adultContent;
  return (
    <div role="button" tabIndex={0} className={`mcard panel${c.hidden || c.ignored ? " off" : ""}${compact ? " compact" : ""}${rows.length ? " have" : ""}`} onClick={onOpen}
      onKeyDown={(e) => { if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); onOpen(); } }} aria-label={c.name}>
      <span className="ph"><span className="pt"><Icon name="browse" size={9} />{c.category}</span><span className="sp" /><span className="cnt">↓ {formatCount(c.downloads)}</span></span>
      <span className={`mcard-img${veil ? " veiled" : ""}`} title={veil ? "Adult, open to show" : undefined}>
        {veil && <span className="mcard-badge l"><Pill tone="over">18+</Pill></span>}
        {c.thumbnailUrl ? <img src={c.thumbnailUrl} alt="" loading="lazy" /> : <Icon name="file" size={27} />}
        {c.downloaded && !rows.length && <span className="mcard-badge"><Pill tone="ok">Downloaded</Pill></span>}
        {ctx && <span className="mcard-q">
          <ModKey mod={{ modId: c.modId, name: c.name, version: c.version }} rows={rows} queued={ctx.queued.get(c.modId)} updates={ctx.updates}
            onDetail={onOpen} onLibrary={ctx.onLibrary} onFlag={(f, on) => ctx.onFlag(c, f, on)} flags={{ hidden: c.hidden, ignored: c.ignored }} />
        </span>}
      </span>
      <span className="mcard-b">
        <span className="mcard-t">{c.name}</span>
        <span className="m-meta"><span>{c.author ?? c.uploader?.name ?? "Unknown"}</span><span>★ {formatCount(c.endorsements)}</span><span>{shortDate(c.updatedAt)}</span></span>
        {!compact && <span className="mcard-s">{c.summary}</span>}
        <span className="m-tags"><AiTags tags={c.tags} /><TypeTag c={c.classification} /><FrameworkTags c={c.classification} /></span>
      </span>
    </div>
  );
}

function FrontPage({ patch, seed, onReshuffle, onOpen, onSeeAll, categories, loading, onCategory, includeAdult }: {
  patch: PatchOpt | null; seed: number; onReshuffle: () => void; onOpen: (c: CardView) => void; onSeeAll: (q: Partial<BrowseQuery>) => void;
  categories: Array<[string, number]>; loading?: boolean; onCategory: (c: string) => void; includeAdult: boolean;
}) {
  // shelves from extensions come after the first two of the app's own
  const fromAddons = useExtensions().active.shelves.map((s) => ({ id: `ext:${s.extId}:${s.id}`, title: s.title, icon: (s.icon && s.icon in ICONS ? s.icon : "addons") as IconName,
    hint: s.hint ? `${s.hint} · ${s.extName}` : `From ${s.extName}`, q: shelfQuery(s.query) }));
  const base = shelves(patch, seed);
  const list = [...base.slice(0, 2), ...fromAddons, ...base.slice(2)];
  return (
    <div className="stack">
      {(categories.length > 0 || loading) && (
        <section>
          <div className="grp" style={{ paddingTop: 0 }}>Categories {categories.length > 0 && <span>{categories.length}</span>}</div>
          <div className="cat-tiles">
            {categories.length === 0 && Array.from({ length: 7 }, (_, i) => <span key={i} className="skel" style={{ height: "3.1em" }} aria-hidden="true" />)}
            {categories.slice(0, 14).map(([name, n]) => (
              <button key={name} type="button" className="cat" onClick={() => onCategory(name)}><b>{name}</b><span>{formatCount(n)}</span></button>
            ))}
          </div>
        </section>
      )}
      {list.map((s) => <Shelf key={`${s.id}-${s.id === "surprise" ? seed : ""}`} title={s.title} icon={s.icon} hint={s.hint} q={{ ...emptyQuery(), ...s.q, includeAdult, count: 12 }}
        onOpen={onOpen} onSeeAll={() => onSeeAll(s.q)} extra={s.id === "surprise" ? <Key size="sm" variant="ghost" icon="refresh" onClick={onReshuffle}>Shuffle</Key> : undefined} />)}
    </div>
  );
}

function Shelf({ title, icon, hint, q: asked, onOpen, onSeeAll, extra }: { title: string; icon: IconName; hint: string; q: BrowseQuery; onOpen: (c: CardView) => void; onSeeAll: () => void; extra?: React.ReactNode }) {
  const [r, setR] = useState<BrowseResult | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const q = withAiHidden(asked, useAiHide());
  const key = JSON.stringify(q);
  const empty = r !== null && r.nodes.length === 0;
  useEffect(() => {
    let live = true;
    setErr(null);
    browse.search(q, { showHidden: false, downloaded: null, skipCounts: true }).then((x) => live && setR(x)).catch((e) => live && setErr(errorText(e)));
    return () => { live = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  if (empty && !err) return null;
  return (
    <section className="shelf">
      <div className="shelf-h">
        <Icon name={icon} />
        <span><b className="hi">{title}</b><span className="dim"> · {hint}</span></span>
        <span className="sp" />
        {extra}
        <Key size="sm" variant="ghost" icon="chevron" onClick={onSeeAll}>See all</Key>
      </div>
      {err && <p className="dim">{err}</p>}
      <div className="shelf-row">
        {r ? r.nodes.map((c) => <ModCard key={c.modId} c={c} compact onOpen={() => onOpen(c)} />)
          : !err && Array.from({ length: 6 }, (_, i) => <span key={i} className="mcard panel compact skel" aria-hidden="true" />)}
      </div>
    </section>
  );
}

/** A big card: the picture behind the name, like a storefront's front row. */
function HeroCard({ c, onOpen }: { c: CardView; onOpen: () => void }) {
  const veil = useAdult() === "blur" && !!c.adultContent;
  return (
    <button type="button" className="hero panel" onClick={onOpen}>
      <span className="ph"><span className="pt"><Icon name="up" size={9} />{c.category}</span><span className="sp" /><span className="cnt">★ {formatCount(c.endorsements)}</span></span>
      <span className={`hero-pic${veil ? " veiled" : ""}`}>
        {c.thumbnailUrl && <img src={c.thumbnailUrl.replace("/thumbnails/", "/")} alt="" loading="lazy" onError={(e) => { (e.target as HTMLImageElement).src = c.thumbnailUrl!; }} />}
        <span className="hero-txt">
          <b>{c.name}</b>
          <span>by {c.author ?? c.uploader?.name ?? "unknown"} · ↓ {formatCount(c.downloads)}</span>
          <span className="hero-sum">{c.summary}</span>
          <span className="m-tags"><AiTags tags={c.tags} /></span>
        </span>
      </span>
    </button>
  );
}
