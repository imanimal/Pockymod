// Collections: browse Nexus collections, open one as a page (its picture, description, media and every mod in it,
// against what you have) or check it quickly before installing; make, export and import local ones.
import { descriptionLinks, descriptionParts, descriptionText, plural } from "../lib/text";
import { useEffect, useMemo, useRef, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Chip, Empty, Key, Lcd, Module, Pill, SearchField, Seg, Switch, Veil, bytesText, formatCount, useBusy, useProgress } from "../ui/pocket";
import { held } from "../lib/tauri";
import { useAdult } from "../lib/prefs";
import {
  collections, errorText, links as linksApi, nexus, nexusPages, nexusUrl, openExternal, pickCollectionFile, pickSavePath, settings,
  type CollectionCard, type CollectionCheck, type CollectionsPage, type LocalCollection,
} from "../lib/api";
import { toast } from "../ui/toast";
import { PremiumNote } from "../ui/Premium";
import { DescText, Links } from "./browse/Links";
import { RichText } from "./browse/RichText";

const inBrowser = (slug: string) => openExternal(nexusUrl.collection(slug)).catch((e) => toast(errorText(e), true));
const onNexus = (slug: string) => nexusPages.url(nexusUrl.collection(slug)).catch((e) => toast(errorText(e), true));

type Tab = "nexus" | "mine";

export function Collections({ onQueued }: { onQueued: () => void }) {
  const [tab, setTab] = useState<Tab>("nexus");
  return (
    <div className="stack">
      <Seg label="Collections" value={tab} onChange={setTab} options={[{ value: "nexus", label: "On Nexus" }, { value: "mine", label: "Mine" }]} />
      {tab === "nexus" ? <NexusCollections onQueued={onQueued} /> : <MyCollections onQueued={onQueued} />}
    </div>
  );
}

function NexusCollections({ onQueued }: { onQueued: () => void }) {
  const [text, setText] = useState("");
  const adult = useAdult();
  const [q, setQ] = useState({ text: "", gameVersion: null as string | null, sort: "endorsements", offset: 0 });
  const [page, setPage] = useState<CollectionsPage | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  // the pager key that asked for the page on its way, and a new try after a failure
  const [paging, setPaging] = useState<"prev" | "next" | null>(null);
  const [tries, setTries] = useState(0);
  const [open, setOpen] = useState<CollectionCard | null>(null);
  const [shown, setShown] = useState<CollectionCard | null>(null);
  const listTop = useRef(0);
  const viewEl = () => document.querySelector<HTMLElement>(".shell > .view");
  const show = (c: CollectionCard | null) => {
    if (c) { listTop.current = viewEl()?.scrollTop ?? 0; requestAnimationFrame(() => viewEl()?.scrollTo({ top: 0 })); }
    else requestAnimationFrame(() => viewEl()?.scrollTo({ top: listTop.current }));
    setShown(c);
  };
  useEffect(() => { const t = setTimeout(() => setQ((o) => ({ ...o, text, offset: 0 })), 350); return () => clearTimeout(t); }, [text]);
  useEffect(() => {
    // only the newest request lands, so a slow page never replaces a newer search
    let live = true;
    setLoading(true);
    collections.browse({ ...q, includeAdult: adult !== "hide" })
      .then((p) => {
        if (!live) return;
        setPage(p); setErr(null);
        // a new page starts at its top, not where the pager was
        if (paging) viewEl()?.scrollTo({ top: 0 });
      })
      .catch((e) => { if (live) setErr(errorText(e)); })
      .finally(() => { if (live) { setLoading(false); setPaging(null); } });
    return () => { live = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [q, adult, tries]);
  const turn = (dir: "prev" | "next") => { setPaging(dir); setQ({ ...q, offset: dir === "prev" ? Math.max(0, q.offset - 24) : q.offset + 24 }); };
  const versions = Object.entries(page?.facetsData?.gameVersion ?? {}).filter(([v]) => v).sort((a, b) => b[1] - a[1]).slice(0, 8);

  useEffect(() => { const b = () => { if (shown) show(null); }; window.addEventListener("pm-back", b); return () => window.removeEventListener("pm-back", b); });
  if (shown) return <CollectionPage c={shown} onBack={() => show(null)} onQueued={() => { show(null); onQueued(); }} />;
  return (
    <>
      <div className="browse-top">
        <SearchField value={text} onChange={setText} placeholder="Search Cyberpunk 2077 collections" />
        <Seg label="Sort" value={q.sort} onChange={(sort) => setQ({ ...q, sort, offset: 0 })}
          options={[{ value: "endorsements", label: "Endorsed" }, { value: "downloads", label: "Downloads" }, { value: "updatedAt", label: "Updated" }, { value: "rating", label: "Rating" }]} />
      </div>
      <div className="chips">
        <span className="dim" style={{ fontSize: ".82em", alignSelf: "center" }}>Game version</span>
        {versions.map(([v, n]) => <Chip key={v} on={q.gameVersion === v} count={n} onClick={() => setQ({ ...q, gameVersion: q.gameVersion === v ? null : v, offset: 0 })}>{v}</Chip>)}
      </div>
      {err && <div className="note bad">{err} <button type="button" className="linkish" onClick={() => setTries((n) => n + 1)}>Try again</button></div>}
      {page && !err && page.nodes.length === 0 && <Empty icon="collections" title="No collections match" />}
      <div className="grid3">
        {!page && loading && Array.from({ length: 6 }, (_, i) => <span key={i} className="skel" style={{ height: "17em" }} aria-hidden="true" />)}
        {page?.nodes.map((c) => {
          const r = c.latestPublishedRevision;
          return (
            <div key={c.id} className="ccard" role="button" tabIndex={0} aria-label={`${c.name}: open its page`} onClick={() => show(c)}
              onKeyDown={(e) => { if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); show(c); } }}>
            <Module icon="collections" title={c.name} count={r ? plural(r.modCount, "mod") : undefined}
              tools={<span className="actions" onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
                <Key size="sm" variant="ghost" icon="link" onClick={() => inBrowser(c.slug)} aria-label={`Open ${c.name} in your browser`} title="Open in your browser" />
                <Key size="sm" icon="check" onClick={() => setOpen(c)} title="What you have, what's missing, clicks and size">Check</Key>
              </span>}>
              {c.tileImage && <img className={`hero-img${adult === "blur" && (c.adultContent || r?.adultContent) ? " veiled-img" : ""}`} src={c.tileImage.thumbnailUrl ?? c.tileImage.url} alt="" loading="lazy" />}
              <b className="hi" style={{ display: "block", marginTop: ".5em" }}>{c.name}</b>
              <div className="m-meta"><span>{c.user.name}</span>{c.category && <span>{c.category.name}</span>}</div>
              <p style={{ fontSize: ".88em", margin: ".3em 0", display: "-webkit-box", WebkitLineClamp: 2, WebkitBoxOrient: "vertical", overflow: "hidden" }}>{c.summary}</p>
              <div className="actions">
                {r?.gameVersions.map((g) => <Pill key={g.reference}>{g.reference}</Pill>)}
                <span className="mono dim" style={{ fontSize: ".78em" }}>★ {formatCount(c.endorsements)} · ↓ {formatCount(c.totalDownloads)}{r ? ` · ${bytesText(r.totalSize)}` : ""}</span>
              </div>
            </Module>
            </div>
          );
        })}
      </div>
      {page && page.totalCount > 24 && (
        <div className="pager">
          <Key size="sm" icon="up" busy={paging === "prev"} disabled={q.offset === 0 || loading} onClick={() => turn("prev")} aria-label="Previous page" title="Previous page" />
          <div className="display sm inline"><span className="d-lbl">Page</span><span className="d-num">{String(q.offset / 24 + 1).padStart(2, "0")}<small>/{Math.ceil(page.totalCount / 24)}</small></span></div>
          <Key size="sm" icon="down" busy={paging === "next"} disabled={q.offset + 24 >= page.totalCount || loading} onClick={() => turn("next")} aria-label="Next page" title="Next page" />
        </div>
      )}
      {open && <CheckDialog title={open.name} load={() => collections.check(open.slug)} onClose={() => setOpen(null)} onQueued={() => { setOpen(null); onQueued(); }} />}
    </>
  );
}

const nameOf = (c: CollectionCheck, modId: number) => c.entries.find((e) => e.modId === modId)?.name ?? `mod ${modId}`;

/** Pre-install checks, then Install. */
function CheckDialog({ title, load, onClose, onQueued }: { title: string; load: () => Promise<CollectionCheck>; onClose: () => void; onQueued: () => void }) {
  return <Dialog icon="collections" title={title} onClose={onClose}><CheckBody load={load} onClose={onClose} onQueued={onQueued} /></Dialog>;
}

/** The checks and Install, in the quick dialog or beside a collection's page (`all` lists the mods you have too). */
function CheckBody({ load, onClose, onQueued, all }: { load: () => Promise<CollectionCheck>; onClose?: () => void; onQueued: () => void; all?: boolean }) {
  const [c, setC] = useState<CollectionCheck | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [optional, setOptional] = useState(false);
  const [premium, setPremium] = useState(false);
  useEffect(() => { nexus.status().then((s) => setPremium(!!s.user?.isPremium)).catch(() => {}); }, []);
  const [busy, setBusy] = useState(false);
  const [tries, setTries] = useState(0);
  // how many of its files have been looked up on Nexus so far
  const [prog, setProg] = useState<[number, number] | null>(null);
  useEffect(() => (c ? undefined : held(collections.onProgress(setProg))), [c]);
  // load when opened, and again on Try again (the caller passes a new function each render)
  useEffect(() => {
    let live = true;
    setErr(null);
    load().then((x) => { if (live) setC(x); }).catch((e) => { if (live) setErr(errorText(e)); });
    return () => { live = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tries]);
  // what Install queues: missing mods (optional ones only when asked) and removed ones that have a replacement
  const toQueue = c ? c.entries.filter((e) => e.status !== "installed" && (optional || !e.optional) && (e.status !== "unavailable" || e.replacement)).length : 0;
  const install = async () => {
    if (!c) return;
    setBusy(true);
    try { await collections.install(c, optional); toast(`${c.name} queued, ${plural(toQueue, "mod")}`); onQueued(); } catch (e) { toast(errorText(e), true); setBusy(false); }
  };
  const copy = (url: string) => navigator.clipboard.writeText(url).then(() => toast("Link copied")).catch(() => toast("Couldn't copy", true));
  const shown = (all ? c?.entries : c?.entries.filter((e) => e.status !== "installed")) ?? [];
  return (
    <>
      {err && <div className="note bad">{err} <button type="button" className="linkish" onClick={() => setTries((n) => n + 1)}>Try again</button></div>}
      {!c && !err && <Empty icon="clock" title="Checking">{prog && prog[1] > 0 ? `${prog[0]} of ${prog[1]} files` : undefined}</Empty>}
      {c && <>
        <div className="lcds">
          <Lcd size="sm" label="Missing" value={String(c.missing).padStart(2, "0")} sub={`${c.installed} installed`} />
          <Lcd size="sm" label="Needs your click" value={String(c.needsClick).padStart(2, "0")} tone={c.needsClick ? "hot" : "ok"} sub={c.needsClick ? (premium ? "Manual downloads" : "Free account") : "Automatic"} />
          <Lcd size="sm" label="Unavailable" value={String(c.unavailable).padStart(2, "0")} tone={c.unavailable ? "bad" : "ok"} sub="Removed on Nexus" />
          <Lcd size="sm" label="Download" value={bytesText(c.downloadBytes).split(" ")[0]} unit={bytesText(c.downloadBytes).split(" ")[1]} />
        </div>
        <div className="actions">
          {c.gameVersionOk === true && <Pill tone="ok">Made for {c.collectionVersions.join(", ")}, matches yours</Pill>}
          {c.gameVersionOk === false && <Pill tone="over">Made for {c.collectionVersions.join(", ")}, you have {c.currentPatch}</Pill>}
          {c.gameVersionOk === null && <Pill tone="soon">Game version {c.collectionVersions.join(", ") || "unknown"}</Pill>}
        </div>
        {c.gameVersionOk === false && <div className="note warn">Made for another patch. Check Health after installing</div>}
        {c.manifest && (c.manifest.withChoices > 0 || c.manifest.wins.length > 0) && (
          <p className="dim" style={{ fontSize: ".86em" }} title="From the collection's manifest. Your own picks stay">
            <span className="okc">Curator setup</span> {c.manifest.withChoices > 0 && <>installer options for {plural(c.manifest.withChoices, "mod")}</>}{c.manifest.withChoices > 0 && c.manifest.wins.length > 0 && ", "}
            {c.manifest.wins.length > 0 && <>{plural(c.manifest.wins.length, "file rule")}</>}</p>
        )}
        {c.manifest && c.manifest.conflicts.length > 0 && (
          <div className="note warn">Marked incompatible by the curator: {c.manifest.conflicts.slice(0, 4).map(([a, b]) => `${nameOf(c, a)} + ${nameOf(c, b)}`).join(", ")}</div>
        )}
        {c.needsClick > 20 && <div className="note warn">{c.needsClick} clicks on Nexus, one page at a time</div>}
        <PremiumNote clicks={c.needsClick} what="this collection" from="collection" />
        <div className="rows" style={all ? undefined : { maxHeight: "16em", overflow: "auto" }}>
          {shown.slice(0, all ? 400 : 80).map((e) => (
            <div key={`${e.modId}-${e.fileId}`} className={`row${e.status === "unavailable" && !e.replacement ? " off" : ""}`}>
              <span className="m-main"><span className="m-t">{e.name}</span>
                <span className="m-meta"><span>{e.fileName}</span><span>v{e.version}</span>{e.optional && <span>optional</span>}
                  {e.replacement && <span>file gone, using v{e.replacement[1]}</span>}</span>
                {c.manifest?.instructions[String(e.modId)] && <span style={{ fontSize: ".82em", whiteSpace: "normal" }} title="Curator's note">{c.manifest.instructions[String(e.modId)]}</span>}</span>
              <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".4em" }}>
                <span>{bytesText(e.size)}</span>
                {e.status === "installed" && <Pill tone="ok">You have it</Pill>}
                {e.status === "other" && <Pill tone="soon">Other version installed</Pill>}
                {e.status === "unavailable" && <Pill tone={e.replacement ? "soon" : "over"}>{e.replacement ? "Replaced" : "Unavailable"}</Pill>}
              </span>
            </div>
          ))}
          {shown.length > (all ? 400 : 80) && <p className="dim" style={{ padding: ".5em" }}>and {shown.length - (all ? 400 : 80)} more</p>}
        </div>
        {(c.external?.length ?? 0) > 0 && (
          <div className="note warn">
            <b>{c.external!.length} from outside Nexus</b>, download {c.external!.length === 1 ? "it" : "them"} yourself
            <div className="rows" style={{ marginTop: ".4em" }}>
              {c.external!.map((x) => (
                <div key={x.name} className="row" style={{ cursor: "default" }}>
                  <span className="m-main"><span className="m-t">{x.name}</span>
                    <span className="m-meta">{x.version && <span>v{x.version}</span>}{x.optional && <span>optional</span>}{x.resourceType && <span>{x.resourceType.toLowerCase()}</span>}</span>
                    {x.instructions && <span style={{ fontSize: ".84em", whiteSpace: "normal" }}>{x.instructions}</span>}</span>
                  {x.resourceUrl && <span className="m-side" style={{ flexDirection: "row", gap: ".3em" }}>
                    <Key size="sm" variant="ghost" icon="web" onClick={() => void linksApi.open(x.resourceUrl!).catch((e) => toast(errorText(e), true))} title={`${x.resourceUrl}\nOpens in your browser`}>Open</Key>
                    <Key size="sm" variant="ghost" icon="file" onClick={() => void copy(x.resourceUrl!)} aria-label={`Copy ${x.resourceUrl}`} title="Copy link" />
                  </span>}
                </div>
              ))}
            </div>
          </div>
        )}
        <div className="actions" style={{ justifyContent: "flex-end" }}>
          {c.entries.some((e) => e.optional) && <Switch checked={optional} onChange={setOptional}>Include optional mods</Switch>}
          {onClose && <Key variant="ghost" onClick={onClose}>Cancel</Key>}
          <Key variant="primary" icon="install" busy={busy} disabled={toQueue === 0} onClick={install}>{toQueue ? `Install ${plural(toQueue, "mod")}` : c.missing || c.unavailable ? "Nothing to install" : "All installed"}</Key>
        </div>
      </>}
    </>
  );
}

/** A collection as a page, like its Nexus page: the picture across the top, the description and its media, and
 *  beside them every mod in it against what you have, with Install. */
function CollectionPage({ c, onBack, onQueued }: { c: CollectionCard; onBack: () => void; onQueued: () => void }) {
  const [more, setMore] = useState<{ description: string | null; headerImage: string | null } | null>(null);
  // without the full description the summary stands in for it, so a failure here needs no message
  useEffect(() => {
    let live = true;
    setMore(null);
    collections.detail(c.slug).then((x) => { if (live) setMore(x); }).catch(() => { if (live) setMore({ description: null, headerImage: null }); });
    return () => { live = false; };
  }, [c.slug]);
  // Escape goes back to the list, unless a viewer or a dialog is open over the page
  useEffect(() => {
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape" && !document.querySelector(".lightbox, .ov, .pmenu.popover")) onBack(); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onBack]);
  const r = c.latestPublishedRevision;
  const raw = more?.description ?? "";
  const parts = useMemo(() => descriptionParts(raw || c.summary || ""), [raw, c.summary]);
  const desc = raw ? descriptionText(raw) : c.summary;
  const links = useMemo(() => descriptionLinks(parts), [parts]);
  const banner = more?.headerImage ?? c.tileImage?.url ?? null;
  const [shown, setShown] = useState(false);
  const veil = { on: useAdult() === "blur" && !!(c.adultContent || r?.adultContent) && !shown, onShow: () => setShown(true) };
  return (
    <div className="mpage-wrap">
      <div className="actions"><Key size="sm" variant="ghost" icon="chevron" className="flip" onClick={onBack}>Back to collections</Key></div>
      <Module icon="collections" title={c.name} count={r ? plural(r.modCount, "mod") : undefined} className="mpage"
        tools={<>
          <Key size="sm" variant="ghost" icon="web" onClick={() => onNexus(c.slug)} title="Nexus page" aria-label="Open the Nexus page" />
          <Key size="sm" variant="ghost" icon="link" onClick={() => inBrowser(c.slug)} title="Open in your browser" aria-label="Open in browser" />
          <Key size="sm" variant="ghost" icon="x" onClick={onBack} aria-label="Close" title="Back to collections (Esc)" />
        </>}>
        <div className="mpage-banner">
          {banner && <img src={banner} alt="" className={veil.on ? "veiled-img" : undefined} />}
          <div className="mpage-head">
            <h2>{c.name}</h2>
            <div className="m-meta"><span>by {c.user.name}</span>{c.category && <span>{c.category.name}</span>}{r && <span>revision {r.revisionNumber}</span>}</div>
            {c.summary && <p>{c.summary}</p>}
            <div className="actions">{r?.gameVersions.map((g) => <Pill key={g.reference}>Game {g.reference}</Pill>)}{(c.adultContent || r?.adultContent) && <Pill tone="over">Adult content</Pill>}
              <Key size="sm" variant="ghost" icon="web" onClick={() => onNexus(c.slug)} title="Comments, bugs and revisions">View on Nexus</Key></div>
          </div>
        </div>
        <div className="mpage-body">
          <div className="mpage-main">
            <div className="lcds">
              <Lcd size="sm" label="Mods" value={String(r?.modCount ?? 0).padStart(2, "0")} />
              {r && <Lcd size="sm" label="Size" value={bytesText(r.totalSize).split(" ")[0]} unit={bytesText(r.totalSize).split(" ")[1]} />}
              <Lcd size="sm" label="Endorsed" value={formatCount(c.endorsements)} />
              <Lcd size="sm" label="Downloads" value={formatCount(c.totalDownloads)} />
            </div>
            {links.length > 0 && <Links key={c.slug} list={links} />}
            <div className="sub-h">Description</div>
            {more === null ? (
              <div className="stack" style={{ gap: ".55em" }} aria-hidden="true">
                <span className="skel skel-line" style={{ width: "92%" }} />
                <span className="skel skel-line" style={{ width: "80%" }} />
                <span className="skel skel-line" style={{ width: "86%" }} />
                <span className="skel skel-line" style={{ width: "40%" }} />
              </div>
            ) : <Veil {...veil}><div className="desc rich">{desc ? (raw ? <RichText src={raw} /> : <DescText parts={parts} />) : <span className="dim">No description</span>}</div></Veil>}
          </div>
          <aside className="mpage-side">
            <div className="sub-h">Before you install</div>
            <CheckBody load={() => collections.check(c.slug)} onQueued={onQueued} all />
          </aside>
        </div>
      </Module>
    </div>
  );
}

function MyCollections({ onQueued }: { onQueued: () => void }) {
  const [name, setName] = useState("");
  const [desc, setDesc] = useState("");
  const [made, setMade] = useState<LocalCollection | null>(null);
  const [imported, setImported] = useState<{ c: LocalCollection; check: CollectionCheck } | null>(null);
  const [publish, setPublish] = useState(false);
  const [creating, runCreate] = useBusy();
  const [exporting, runExport] = useBusy();
  const [opening, runOpen] = useBusy();
  const importP = useProgress(opening, collections.onProgress);
  useEffect(() => { settings.get("flags.publishCollections").then((v) => setPublish(v === "1")).catch(() => {}); }, []);
  const create = () => { if (name.trim() && !creating) void runCreate(async () => setMade(await collections.createLocal(name, desc))); };
  const enter = (e: React.KeyboardEvent) => { if (e.key === "Enter") create(); };

  return (
    <div className="grid2" style={{ alignItems: "start" }}>
      <Module icon="share" title="Make a collection" count="from the active profile">
        <div className="stack" style={{ gap: ".5em" }}>
          <label className="fbox" style={{ flex: "none" }}><input className="finput" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={enter} placeholder="Collection name" aria-label="Collection name" /></label>
          <label className="fbox" style={{ flex: "none" }}><input className="finput" value={desc} onChange={(e) => setDesc(e.target.value)} onKeyDown={enter} placeholder="Description (optional)" aria-label="Description" /></label>
          <div className="actions">
            <Key icon="collections" busy={creating} disabled={!name.trim()} title={name.trim() ? "Collects the mods that are on in the active profile" : "Give it a name first"}
              onClick={create}>Create</Key>
          </div>
        </div>
        {made && <>
          <div className="sub-h">{made.name} · {plural(made.mods.length, "mod")} · game {made.gameVersion ?? "?"}</div>
          <div className="rows" style={{ maxHeight: "14em", overflow: "auto" }}>
            {made.mods.map((m) => (
              <div key={m.modId} className="row">
                <span className="m-main"><span className="m-t">{m.name}</span>
                  <span className="m-meta"><span>file {m.fileId}</span><span className="mono">{m.md5 ? m.md5.slice(0, 10) : "no hash"}</span>{m.requires.length > 0 && <span>needs {m.requires.length}</span>}</span></span>
                <span className="m-side"><span>{m.size ? bytesText(m.size) : ""}</span></span>
              </div>
            ))}
          </div>
          <div className="actions" style={{ marginTop: ".6em" }}>
            <Key variant="primary" icon="share" busy={exporting} onClick={() => runExport(async () => {
              const path = await pickSavePath(`${made.name.replace(/[^\w -]/g, "_")}.pmcollection.json`);
              if (path) { await collections.exportLocal(made, path); toast("Collection saved"); }
            })}>Export</Key>
            <Key variant="ghost" disabled={!publish} title={publish ? "Experimental" : "Turn on the experiment below"}
              onClick={() => toast("Not available yet. Nexus hasn't approved third-party publishing", true)}>Publish to Nexus</Key>
          </div>
        </>}
        <div className="sub-h">Experiments</div>
        <Switch checked={publish} onChange={(v) => { setPublish(v); settings.set("flags.publishCollections", v ? "1" : "0").catch((e) => { setPublish(!v); toast(errorText(e), true); }); }}>Publish collections to Nexus (does nothing yet)</Switch>
      </Module>

      <Module icon="file" title="Import a collection" count={imported ? imported.c.name : undefined}>
        <div className="actions">
          <Key icon="file" busy={opening && (importP ?? true)} onClick={() => runOpen(async () => {
            const path = await pickCollectionFile();
            if (path) { const [c, check] = await collections.importLocal(path); setImported({ c, check }); }
          })}>Open a file</Key>
        </div>
        {imported && <CheckDialog title={imported.c.name} load={() => Promise.resolve(imported.check)} onClose={() => setImported(null)} onQueued={() => { setImported(null); onQueued(); }} />}
        {!imported && <Empty icon="collections" title="Nothing imported">.pmcollection.json files</Empty>}
      </Module>
    </div>
  );
}
