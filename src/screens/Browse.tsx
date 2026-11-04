// Browse: search on top, a filter rail of chips with counts, the result rows, and a detail module whose
// Install key is the screen's one orange key.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "../ui/Icon";
import { Chip, Empty, Key, Lcd, Module, Pill, SearchField, Seg, Switch, Tag, bytesText, formatCount } from "../ui/pocket";
import {
  FRAMEWORKS, MOD_TYPES, browse, errorText, fileBytes, settings,
  type BrowseQuery, type BrowseResult, type CardView, type Classification, type DetailView, type LocalFilter, type ModTypeFilter, type SortKey,
} from "../lib/api";
import { daysAgo, descriptionText, shortDate } from "../lib/text";

const PAGE = 30;
const SORTS: Array<{ value: SortKey; label: string }> = [
  { value: "relevance", label: "Relevance" }, { value: "downloads", label: "Downloads" }, { value: "endorsements", label: "Endorsements" },
  { value: "updated", label: "Updated" }, { value: "created", label: "Created" }, { value: "size", label: "Size" },
];
type Since = "any" | "7" | "30" | "365";
type Dl = "any" | "1000" | "10000" | "100000";
type Size = "any" | "10" | "100" | "1024";

export const emptyQuery = (): BrowseQuery => ({
  text: "", categories: [], tags: [], excludeTags: [], sort: "downloads", ascending: false, includeAdult: false, frameworks: [], modType: null, offset: 0, count: PAGE,
});

export function Browse({ onInstall, onOpenPage }: { onInstall?: (d: DetailView, fileId: number) => void; onOpenPage?: (modId: number) => void }) {
  const [q, setQ] = useState<BrowseQuery>(emptyQuery);
  const [text, setText] = useState("");
  const [local, setLocal] = useState<LocalFilter>({ showHidden: false, downloaded: null });
  const [since, setSince] = useState<Since>("any");
  const [dl, setDl] = useState<Dl>("any");
  const [size, setSize] = useState<Size>("any");
  const [compat, setCompat] = useState(false);
  const [patch, setPatch] = useState<{ version: string; at: number } | null>(null);
  const [res, setRes] = useState<BrowseResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [sel, setSel] = useState<CardView | null>(null);
  const [moreCats, setMoreCats] = useState(false);
  const [moreTags, setMoreTags] = useState(false);

  // the installed patch drives the "works with my game" filter
  useEffect(() => {
    (async () => {
      const list = await browse.patches();
      const saved = await settings.get("game.patch");
      const pick = list.find(([p]) => p.version === saved) ?? list[list.length - 1];
      if (pick?.[1]) setPatch({ version: pick[0].version, at: pick[1] });
    })().catch(() => {});
  }, []);

  // debounce typing
  useEffect(() => {
    const t = setTimeout(() => setQ((old) => (old.text ?? "") === text ? old : { ...old, text, offset: 0, sort: text && old.sort === "downloads" ? "relevance" : !text && old.sort === "relevance" ? "downloads" : old.sort }), 350);
    return () => clearTimeout(t);
  }, [text]);

  const effective = useMemo<BrowseQuery>(() => ({
    ...q,
    updatedAfter: since === "any" ? null : daysAgo(Number(since)),
    downloadsMin: dl === "any" ? null : Number(dl),
    sizeMaxKb: size === "any" ? null : Number(size) * 1024,
    compatibleSince: compat && patch ? patch.at : null,
  }), [q, since, dl, size, compat, patch]);

  const seq = useRef(0);
  useEffect(() => {
    const n = ++seq.current;
    setBusy(true); setErr(null);
    browse.search(effective, local)
      .then((r) => { if (n === seq.current) setRes(r); })
      .catch((e) => { if (n === seq.current) setErr(errorText(e)); })
      .finally(() => { if (n === seq.current) setBusy(false); });
  }, [effective, local]);

  const set = (patch: Partial<BrowseQuery>) => setQ((old) => ({ ...old, ...patch, offset: patch.offset ?? 0 }));
  const toggle = <T,>(list: T[], v: T) => (list.includes(v) ? list.filter((x) => x !== v) : [...list, v]);
  const derived = useMemo(() => new Map(res?.derived ?? []), [res]);

  const flag = useCallback(async (c: CardView, which: "hidden" | "ignored", on: boolean) => {
    await browse.setFlag(c.modId, which, on);
    setRes((r) => r && ({ ...r, nodes: r.nodes.map((n) => n.modId === c.modId ? { ...n, [which]: on } : n).filter((n) => local.showHidden || !n.hidden) }));
    if (which === "hidden" && on && !local.showHidden) setSel(null);
  }, [local.showHidden]);

  const page = res ? Math.floor(res.offset / PAGE) + 1 : 1;
  const pages = res ? Math.max(1, Math.ceil(res.total / PAGE)) : 1;
  const activeCount = q.categories.length + q.tags.length + q.excludeTags.length + q.frameworks.length + (q.modType ? 1 : 0)
    + (since !== "any" ? 1 : 0) + (dl !== "any" ? 1 : 0) + (size !== "any" ? 1 : 0) + (compat ? 1 : 0) + (local.downloaded !== null ? 1 : 0);

  return (
    <div className="browse">
      <div className="browse-top">
        <SearchField value={text} onChange={setText} placeholder="Search Cyberpunk 2077 mods by name or description" />
        <Seg label="Sort" value={q.sort} onChange={(s) => set({ sort: s })} options={SORTS.filter((s) => s.value !== "relevance" || text)} />
        <Key size="sm" icon={q.ascending ? "up" : "down"} onClick={() => set({ ascending: !q.ascending })} aria-label={q.ascending ? "Ascending" : "Descending"} title={q.ascending ? "Ascending" : "Descending"} />
      </div>

      <div className="split">
        <Module icon="filter" title="Filters" count={activeCount ? `${activeCount} on` : undefined} className="filters"
          tools={activeCount ? <Key size="sm" variant="ghost" onClick={() => { setQ({ ...emptyQuery(), text }); setSince("any"); setDl("any"); setSize("any"); setCompat(false); setLocal({ showHidden: false, downloaded: null }); }}>Clear</Key> : undefined}>
          <div className="sub-h">Needs framework</div>
          <div className="chips">
            {FRAMEWORKS.map((f) => (
              <Chip key={f.key} on={q.frameworks.includes(f.key)} count={derived.get(`fw_${f.key}`)} onClick={() => set({ frameworks: toggle(q.frameworks, f.key) })} title={f.name}>{f.short}</Chip>
            ))}
          </div>
          <div className="sub-h">Mod type</div>
          <div className="chips">
            {MOD_TYPES.map((t) => (
              <Chip key={t.key} on={q.modType === t.key} count={derived.get(`type_${t.key}`)} onClick={() => set({ modType: q.modType === t.key ? null : t.key as ModTypeFilter })}>{t.name}</Chip>
            ))}
          </div>
          <p className="dim hint">Framework and type are worked out from requirements, tags, text and file lists, not given by Nexus.</p>

          <div className="sub-h">Game version</div>
          <Switch checked={compat} onChange={setCompat} disabled={!patch}>Updated since patch {patch?.version ?? "?"}</Switch>

          <div className="sub-h">Updated</div>
          <Seg label="Updated" value={since} onChange={setSince} options={[{ value: "any", label: "Any" }, { value: "7", label: "Week" }, { value: "30", label: "Month" }, { value: "365", label: "Year" }]} />
          <div className="sub-h">Downloads</div>
          <Seg label="Downloads" value={dl} onChange={setDl} options={[{ value: "any", label: "Any" }, { value: "1000", label: "1k+" }, { value: "10000", label: "10k+" }, { value: "100000", label: "100k+" }]} />
          <div className="sub-h">Size</div>
          <Seg label="Size" value={size} onChange={setSize} options={[{ value: "any", label: "Any" }, { value: "10", label: "<10 MB" }, { value: "100", label: "<100 MB" }, { value: "1024", label: "<1 GB" }]} />

          <div className="sub-h">Category</div>
          <div className="chips">
            {(res?.facets.categories ?? []).slice(0, moreCats ? 99 : 10).map(([name, n]) => (
              <Chip key={name} on={q.categories.includes(name)} count={n} onClick={() => set({ categories: toggle(q.categories, name) })}>{name}</Chip>
            ))}
            {q.categories.filter((c) => !res?.facets.categories.some(([n]) => n === c)).map((c) => <Chip key={c} on count={0} onClick={() => set({ categories: toggle(q.categories, c) })}>{c}</Chip>)}
            {(res?.facets.categories.length ?? 0) > 10 && <Key size="sm" variant="ghost" onClick={() => setMoreCats(!moreCats)}>{moreCats ? "Fewer" : "More"}</Key>}
          </div>
          <div className="sub-h">Tags <span className="dim">· shift-click to exclude</span></div>
          <div className="chips">
            {q.excludeTags.map((t) => <Chip key={`x${t}`} on onClick={() => set({ excludeTags: toggle(q.excludeTags, t) })} title="Excluded">− {t}</Chip>)}
            {(res?.facets.tags ?? []).filter(([t]) => !q.excludeTags.includes(t)).slice(0, moreTags ? 120 : 14).map(([name, n]) => (
              <button key={name} type="button" className={`chip${q.tags.includes(name) ? " on" : ""}`} aria-pressed={q.tags.includes(name)}
                onClick={(e) => e.shiftKey ? set({ excludeTags: toggle(q.excludeTags, name), tags: q.tags.filter((t) => t !== name) }) : set({ tags: toggle(q.tags, name) })}>
                {name}<span className="n">{formatCount(n)}</span>
              </button>
            ))}
            {(res?.facets.tags.length ?? 0) > 14 && <Key size="sm" variant="ghost" onClick={() => setMoreTags(!moreTags)}>{moreTags ? "Fewer" : "More"}</Key>}
          </div>

          <div className="sub-h">On this PC</div>
          <Seg label="Downloaded" value={local.downloaded === null ? "all" : local.downloaded ? "yes" : "no"}
            onChange={(v) => setLocal({ ...local, downloaded: v === "all" ? null : v === "yes" })}
            options={[{ value: "all", label: "All" }, { value: "yes", label: "Downloaded" }, { value: "no", label: "Not downloaded" }]} />
          <div className="stack" style={{ gap: ".5em", marginTop: ".6em" }}>
            <Switch checked={local.showHidden} onChange={(v) => setLocal({ ...local, showHidden: v })}>Show hidden mods</Switch>
            <Switch checked={q.includeAdult} onChange={(v) => set({ includeAdult: v })}>Include adult content</Switch>
          </div>
        </Module>

        <Module icon="browse" title="Mods" count={res ? `${formatCount(res.total)}${res.fromCache ? " · cached" : ""}` : busy ? "loading" : undefined} className="results" bodyClass="list"
          tools={<span className={`led${busy ? " run" : ""}`} aria-label={busy ? "Loading" : "Idle"} />}>
          {err && <div className="note bad" style={{ margin: ".6em" }}>{err}</div>}
          {res && res.nodes.length === 0 && !busy && <Empty icon="browse" title="No mods match">Try fewer filters{res.filteredOnPage ? ` (${res.filteredOnPage} on this page are hidden or downloaded)` : ""}.</Empty>}
          <div className="rows">
            {res?.nodes.map((c, i) => <ModRow key={c.modId} c={c} i={i} sel={sel?.modId === c.modId} onSel={() => setSel(c)} />)}
          </div>
          {res && res.total > PAGE && (
            <div className="pager">
              <Key size="sm" icon="up" disabled={res.offset === 0} onClick={() => set({ offset: Math.max(0, res.offset - PAGE) })} aria-label="Previous page" />
              <div className="display sm inline"><span className="d-lbl">Page</span><span className="d-num">{String(page).padStart(2, "0")}<small>/{pages}</small></span></div>
              <Key size="sm" icon="down" disabled={res.offset + PAGE >= res.total} onClick={() => set({ offset: res.offset + PAGE })} aria-label="Next page" />
              {res.filteredOnPage > 0 && <span className="dim" style={{ fontSize: ".82em" }}>{res.filteredOnPage} left out by local filters</span>}
            </div>
          )}
        </Module>

        <div className="detail">
          {sel ? <Detail card={sel} onFlag={flag} onInstall={onInstall} onOpenPage={onOpenPage} onOpen={(id) => setSel({ ...sel, modId: id, name: "", updatedAt: "" } as CardView)} />
            : <Module icon="info" title="Mod" fill><Empty icon="browse" title="Pick a mod">Its files, requirements and the Install key show here.</Empty></Module>}
        </div>
      </div>
    </div>
  );
}

function TypeTag({ c }: { c: Classification }) {
  const label = c.modType === "redmod" ? "REDmod" : c.modType === "unknown" ? "Type unknown" : c.modType[0].toUpperCase() + c.modType.slice(1);
  return <Tag kind={c.modType === "unknown" || c.modType === "mixed" ? undefined : c.modType} square>{label}</Tag>;
}

function FrameworkTags({ c }: { c: Classification }) {
  return <>{c.frameworks.map((f) => <Tag key={f.key} kind={f.key}>{FRAMEWORKS.find((x) => x.key === f.key)?.short ?? f.name}</Tag>)}</>;
}

function ModRow({ c, i, sel, onSel }: { c: CardView; i: number; sel: boolean; onSel: () => void }) {
  return (
    <button type="button" className={`row modrow${sel ? " sel" : ""}${c.hidden || c.ignored ? " off" : ""}`} onClick={onSel} style={{ ["--i" as string]: i }}>
      {c.thumbnailUrl ? <img className="thumb" src={c.thumbnailUrl} alt="" loading="lazy" /> : <span className="thumb ph-img"><Icon name="file" /></span>}
      <span className="m-main">
        <span className="m-t">{c.name}</span>
        <span className="m-meta"><span>{c.author ?? c.uploader?.name ?? "Unknown author"}</span><span>{c.category}</span>
          {c.downloaded && <span><Pill tone="ok">Downloaded</Pill></span>}{c.hidden && <span>Hidden</span>}{c.ignored && <span>Ignored</span>}</span>
        <span className="m-tags"><TypeTag c={c.classification} /><FrameworkTags c={c.classification} /></span>
      </span>
      <span className="m-side">
        <span>{c.fileSize ? bytesText(c.fileSize * 1024) : ""}</span>
        <span>{shortDate(c.updatedAt)}</span>
        <span title={`${c.downloads} downloads`}>↓ {formatCount(c.downloads)}</span>
      </span>
    </button>
  );
}

function Detail({ card, onFlag, onInstall, onOpenPage, onOpen }: {
  card: CardView; onFlag: (c: CardView, f: "hidden" | "ignored", on: boolean) => void;
  onInstall?: (d: DetailView, fileId: number) => void; onOpenPage?: (modId: number) => void; onOpen: (modId: number) => void;
}) {
  const [d, setD] = useState<DetailView | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [fileId, setFileId] = useState<number | null>(null);
  const [full, setFull] = useState(false);
  const [showList, setShowList] = useState(false);

  useEffect(() => {
    let live = true;
    setD(null); setErr(null); setFull(false); setShowList(false);
    browse.detail(card.modId, card.updatedAt || undefined).then((v) => {
      if (!live) return;
      setD(v);
      const main = v.files.filter((f) => f.category === "MAIN").sort((a, b) => b.date - a.date)[0] ?? v.files.filter((f) => ["MAIN", "OPTIONAL", "UPDATE", "MISCELLANEOUS"].includes(f.category)).sort((a, b) => b.date - a.date)[0];
      setFileId(main?.fileId ?? null);
    }).catch((e) => live && setErr(errorText(e)));
    return () => { live = false; };
  }, [card.modId, card.updatedAt]);

  if (err) return <Module icon="warn" title="Mod" band="bad" fill><div className="note bad">{err}</div></Module>;
  if (!d) return <Module icon="info" title="Mod" count="loading" fill><Empty icon="clock" title="Loading" /></Module>;
  const m = d.detail;
  const current = d.files.filter((f) => ["MAIN", "OPTIONAL", "UPDATE", "MISCELLANEOUS"].includes(f.category)).sort((a, b) => b.date - a.date);
  const file = d.files.find((f) => f.fileId === fileId);
  const desc = descriptionText(m.description);
  const view: CardView = { ...card, hidden: d.hidden, ignored: d.ignored };

  return (
    <Module icon="library" title={m.name} count={`v${m.version}`} fill className="detail-mod"
      tools={<Key size="sm" variant="ghost" icon="web" onClick={() => onOpenPage?.(m.modId)} title="Open the Nexus page in the app" aria-label="Open the Nexus page" />}>
      {m.pictureUrl && <img className="hero-img" src={m.pictureUrl} alt="" />}
      <div className="m-meta" style={{ marginTop: ".6em" }}><span>{m.author ?? m.uploader?.name}</span><span>{m.category}</span><span>Updated {shortDate(m.updatedAt)}</span></div>
      <p style={{ margin: ".5em 0 .2em", color: "var(--text)" }}>{m.summary}</p>
      <div className="m-tags"><TypeTag c={d.classification} /><FrameworkTags c={d.classification} /></div>

      <div className="lcds" style={{ marginTop: ".8em" }}>
        <Lcd size="sm" label="Downloads" value={formatCount(m.downloads)} />
        <Lcd size="sm" label="Endorsed" value={formatCount(m.endorsements)} />
        {file && <Lcd size="sm" label="Size" value={bytesText(fileBytes(file)).split(" ")[0]} unit={bytesText(fileBytes(file)).split(" ")[1]} />}
      </div>

      <div className="sub-h">File</div>
      {current.length === 0 ? <p className="dim">No current files. Nexus may have removed them.</p> : (
        <div className="rows">
          {current.slice(0, 8).map((f) => (
            <button key={f.fileId} type="button" className={`row${f.fileId === fileId ? " sel" : ""}`} onClick={() => setFileId(f.fileId)}>
              <span className="m-main"><span className="m-t">{f.name}</span><span className="m-meta"><span>{f.category.toLowerCase().replace("_", " ")}</span><span>v{f.version}</span><span>{shortDate(new Date(f.date * 1000).toISOString())}</span></span></span>
              <span className="m-side"><span>{bytesText(fileBytes(f))}</span></span>
            </button>
          ))}
        </div>
      )}

      <div className="actions" style={{ marginTop: ".8em" }}>
        <Key variant="primary" size="lg" icon="install" disabled={!file} onClick={() => file && onInstall?.(d, file.fileId)}>Install</Key>
        <Key variant="ghost" icon="hide" onClick={() => { onFlag(view, "hidden", !d.hidden); setD({ ...d, hidden: !d.hidden }); }}>{d.hidden ? "Unhide" : "Hide"}</Key>
        <Key variant="ghost" icon="x" onClick={() => { onFlag(view, "ignored", !d.ignored); setD({ ...d, ignored: !d.ignored }); }}>{d.ignored ? "Stop ignoring" : "Ignore"}</Key>
      </div>

      {m.modRequirements.nexusRequirements.totalCount > 0 && <>
        <div className="sub-h">Requires</div>
        <div className="rows">
          {m.modRequirements.nexusRequirements.nodes.map((r) => (
            <button key={`${r.modId}-${r.modName}`} type="button" className="row" onClick={() => !r.externalRequirement && onOpen(r.modId)} disabled={r.externalRequirement}>
              <span className="m-main"><span className="m-t">{r.modName}</span>{r.notes && <span className="m-meta"><span>{r.notes}</span></span>}</span>
              <span className="m-side">{r.externalRequirement ? <span>Off Nexus</span> : <Icon name="chevron" size={9} />}</span>
            </button>
          ))}
        </div>
      </>}

      <div className="sub-h">Files in the archive</div>
      {d.fileList ? (
        <>
          <p className="dim" style={{ fontSize: ".86em" }}>{d.fileList.length} paths indexed by Nexus{d.classification.typeSource === "files" ? ", used for the type" : ""}.{" "}
            <button type="button" className="linkish" onClick={() => setShowList(!showList)}>{showList ? "Hide" : "Show"}</button></p>
          {showList && <div className="log" style={{ maxHeight: "12em" }}>{d.fileList.slice(0, 200).join("\n")}</div>}
        </>
      ) : <p className="dim" style={{ fontSize: ".86em" }}>Nexus has no file list for this file yet. The type is a guess until it is downloaded.</p>}

      <div className="sub-h">Description</div>
      <div className="desc">{full ? desc : desc.slice(0, 900)}{desc.length > 900 && !full && "…"}</div>
      {desc.length > 900 && <Key size="sm" variant="ghost" onClick={() => setFull(!full)}>{full ? "Less" : "Read more"}</Key>}
    </Module>
  );
}
