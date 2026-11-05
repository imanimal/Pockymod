// Collections: browse Nexus collections, open one as a page (its picture, description, media and every mod in it,
// against what you have) or check it quickly before installing; make, export and import local ones.
import { descriptionLinks, descriptionParts, descriptionText, plural } from "../lib/text";
import { useEffect, useMemo, useRef, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { Chip, Empty, Key, Lcd, Module, Pill, SearchField, Seg, Switch, bytesText, formatCount } from "../ui/pocket";
import {
  collections, descriptionMedia, errorText, nexusPages, nexusUrl, openExternal, pickCollectionFile, pickSavePath, settings,
  type CollectionCard, type CollectionCheck, type CollectionsPage, type LocalCollection,
} from "../lib/api";
import { toast } from "../ui/toast";
import { PremiumNote } from "../ui/Premium";
import { MediaStage } from "./browse/Media";
import { DescText, Links } from "./browse/Links";

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
  const [q, setQ] = useState({ text: "", gameVersion: null as string | null, sort: "endorsements", includeAdult: false, offset: 0 });
  const [page, setPage] = useState<CollectionsPage | null>(null);
  const [err, setErr] = useState<string | null>(null);
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
  useEffect(() => { collections.browse(q).then((p) => { setPage(p); setErr(null); }).catch((e) => setErr(errorText(e))); }, [q]);
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
      {err && <div className="note bad">{err}</div>}
      {page && page.nodes.length === 0 && <Empty icon="collections" title="No collections match" />}
      <div className="grid3">
        {page?.nodes.map((c) => {
          const r = c.latestPublishedRevision;
          return (
            <div key={c.id} className="ccard" role="button" tabIndex={0} aria-label={`${c.name}: open its page`} onClick={() => show(c)}
              onKeyDown={(e) => { if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); show(c); } }}>
            <Module icon="collections" title={c.name} count={r ? plural(r.modCount, "mod") : undefined}
              tools={<span className="actions" onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
                <Key size="sm" variant="ghost" icon="link" onClick={() => inBrowser(c.slug)} aria-label={`Open ${c.name} in your browser`} title="Open in your own browser: your sign-in, saved passwords and extensions" />
                <Key size="sm" icon="check" onClick={() => setOpen(c)} title="A quick check: what you have, what is missing, clicks and size">Check</Key>
              </span>}>
              {c.tileImage && <img className="hero-img" src={c.tileImage.thumbnailUrl ?? c.tileImage.url} alt="" loading="lazy" />}
              <b className="hi" style={{ display: "block", marginTop: ".5em" }}>{c.name}</b>
              <div className="m-meta"><span>{c.user.name}</span>{c.category && <span>{c.category.name}</span>}</div>
              <p style={{ fontSize: ".88em", margin: ".3em 0", display: "-webkit-box", WebkitLineClamp: 2, WebkitBoxOrient: "vertical", overflow: "hidden" }}>{c.summary}</p>
              <div className="actions">
                {r?.gameVersions.map((g) => <Pill key={g.reference}>{g.reference}</Pill>)}
                <span className="mono dim" style={{ fontSize: ".78em" }}>★ {formatCount(c.endorsements)} · ↓ {formatCount(c.totalDownloads)} · {r ? bytesText(r.totalSize) : ""}</span>
              </div>
            </Module>
            </div>
          );
        })}
      </div>
      {page && page.totalCount > 24 && (
        <div className="pager">
          <Key size="sm" icon="up" disabled={q.offset === 0} onClick={() => setQ({ ...q, offset: Math.max(0, q.offset - 24) })} aria-label="Previous page" />
          <div className="display sm inline"><span className="d-lbl">Page</span><span className="d-num">{String(q.offset / 24 + 1).padStart(2, "0")}<small>/{Math.ceil(page.totalCount / 24)}</small></span></div>
          <Key size="sm" icon="down" disabled={q.offset + 24 >= page.totalCount} onClick={() => setQ({ ...q, offset: q.offset + 24 })} aria-label="Next page" />
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
  const [busy, setBusy] = useState(false);
  // load once when opened (the caller passes a new function each render)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { load().then(setC).catch((e) => setErr(errorText(e))); }, []);
  const install = async () => {
    if (!c) return;
    setBusy(true);
    try { await collections.install(c, optional); toast(`${c.name} is queued: ${plural(c.missing, "mod")}.`); onQueued(); } catch (e) { toast(errorText(e), true); setBusy(false); }
  };
  const shown = (all ? c?.entries : c?.entries.filter((e) => e.status !== "installed")) ?? [];
  return (
    <>
      {err && <div className="note bad">{err}</div>}
      {!c && !err && <Empty icon="clock" title="Checking the collection" />}
      {c && <>
        <div className="lcds">
          <Lcd size="sm" label="Missing" value={String(c.missing).padStart(2, "0")} sub={`${c.installed} already installed`} />
          <Lcd size="sm" label="Needs your click" value={String(c.needsClick).padStart(2, "0")} tone={c.needsClick ? "hot" : "ok"} sub={c.needsClick ? "Free account" : "Automatic"} />
          <Lcd size="sm" label="Unavailable" value={String(c.unavailable).padStart(2, "0")} tone={c.unavailable ? "bad" : "ok"} sub="Removed on Nexus" />
          <Lcd size="sm" label="Download" value={bytesText(c.downloadBytes).split(" ")[0]} unit={bytesText(c.downloadBytes).split(" ")[1]} />
        </div>
        <div className="actions">
          {c.gameVersionOk === true && <Pill tone="ok">Made for {c.collectionVersions.join(", ")}: matches your {c.currentPatch}</Pill>}
          {c.gameVersionOk === false && <Pill tone="over">Made for {c.collectionVersions.join(", ")}; you have {c.currentPatch}</Pill>}
          {c.gameVersionOk === null && <Pill tone="soon">Game version {c.collectionVersions.join(", ") || "not given"}: cannot compare</Pill>}
        </div>
        {c.gameVersionOk === false && <div className="note warn">Collections made for another patch often need updated frameworks or break. Check Health after installing.</div>}
        {c.manifest && (c.manifest.withChoices > 0 || c.manifest.wins.length > 0) && (
          <p className="dim" style={{ fontSize: ".86em" }} title="From the collection's own manifest: the options its curator picked in each mod's installer, and which mod's files win where two want the same ones. Your own choices are never replaced.">
            <span className="okc">Set up as the curator made it:</span> {c.manifest.withChoices > 0 && <>installer options for {plural(c.manifest.withChoices, "mod")}</>}{c.manifest.withChoices > 0 && c.manifest.wins.length > 0 && ", "}
            {c.manifest.wins.length > 0 && <>{plural(c.manifest.wins.length, "rule")} for which files win</>}. Nothing to pick.</p>
        )}
        {c.manifest && c.manifest.conflicts.length > 0 && (
          <div className="note warn">The curator marks {plural(c.manifest.conflicts.length, "pair")} of mods as clashing: {c.manifest.conflicts.slice(0, 4).map(([a, b]) => `${nameOf(c, a)} and ${nameOf(c, b)}`).join("; ")}.</div>
        )}
        {c.needsClick > 20 && <div className="note warn">That is {c.needsClick} clicks on Nexus. Each page opens in turn, and the queue moves on as soon as you click.</div>}
        <PremiumNote clicks={c.needsClick} />
        <div className="rows" style={all ? undefined : { maxHeight: "16em", overflow: "auto" }}>
          {shown.slice(0, all ? 400 : 80).map((e) => (
            <div key={`${e.modId}-${e.fileId}`} className={`row${e.status === "unavailable" && !e.replacement ? " off" : ""}`}>
              <span className="m-main"><span className="m-t">{e.name}</span>
                <span className="m-meta"><span>{e.fileName}</span><span>v{e.version}</span>{e.optional && <span>optional</span>}
                  {e.replacement && <span>file gone: using v{e.replacement[1]}</span>}</span>
                {c.manifest?.instructions[String(e.modId)] && <span style={{ fontSize: ".82em", whiteSpace: "normal" }} title="The curator's note for this mod">{c.manifest.instructions[String(e.modId)]}</span>}</span>
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
            <b>{c.external!.length} {c.external!.length === 1 ? "item comes" : "items come"} from outside Nexus.</b> The app cannot download {c.external!.length === 1 ? "it" : "them"}; get {c.external!.length === 1 ? "it" : "them"} by hand:
            <div className="rows" style={{ marginTop: ".4em" }}>
              {c.external!.map((x) => (
                <div key={x.name} className="row" style={{ cursor: "default" }}>
                  <span className="m-main"><span className="m-t">{x.name}</span>
                    <span className="m-meta">{x.version && <span>v{x.version}</span>}{x.optional && <span>optional</span>}{x.resourceType && <span>{x.resourceType.toLowerCase()}</span>}</span>
                    {x.instructions && <span style={{ fontSize: ".84em", whiteSpace: "normal" }}>{x.instructions}</span>}</span>
                  {x.resourceUrl && <span className="m-side"><Key size="sm" variant="ghost" icon="web" onClick={() => { void navigator.clipboard?.writeText(x.resourceUrl!); toast("Link copied: open it in your browser."); }}>Copy link</Key></span>}
                </div>
              ))}
            </div>
          </div>
        )}
        <div className="actions" style={{ justifyContent: "flex-end" }}>
          {c.entries.some((e) => e.optional) && <Switch checked={optional} onChange={setOptional}>Include optional mods</Switch>}
          {onClose && <Key variant="ghost" onClick={onClose}>Cancel</Key>}
          <Key variant="primary" icon="install" disabled={busy || c.missing === 0} onClick={install}>{c.missing ? `Install ${plural(c.missing, "mod")}` : "All installed"}</Key>
        </div>
      </>}
    </>
  );
}

/** A collection as a page, like its Nexus page: the picture across the top, the description and its media, and
 *  beside them every mod in it against what you have, with Install. */
function CollectionPage({ c, onBack, onQueued }: { c: CollectionCard; onBack: () => void; onQueued: () => void }) {
  const [more, setMore] = useState<{ description: string | null; headerImage: string | null } | null>(null);
  useEffect(() => { collections.detail(c.slug).then(setMore).catch(() => setMore({ description: null, headerImage: null })); }, [c.slug]);
  const r = c.latestPublishedRevision;
  const raw = more?.description ?? "";
  const parts = useMemo(() => descriptionParts(raw || c.summary || ""), [raw, c.summary]);
  const desc = raw ? descriptionText(raw) : c.summary;
  const links = useMemo(() => descriptionLinks(parts), [parts]);
  const shots = descriptionMedia(raw, null);
  const banner = more?.headerImage ?? c.tileImage?.url ?? null;
  return (
    <div className="mpage-wrap">
      <div className="actions"><Key size="sm" variant="ghost" icon="chevron" className="flip" onClick={onBack}>Back to collections</Key></div>
      <Module icon="collections" title={c.name} count={r ? plural(r.modCount, "mod") : undefined} className="mpage"
        tools={<>
          <Key size="sm" variant="ghost" icon="web" onClick={() => onNexus(c.slug)} title="The collection's Nexus page, where Settings says pages open" aria-label="Open the Nexus page" />
          <Key size="sm" variant="ghost" icon="link" onClick={() => inBrowser(c.slug)} title="Open in your own browser: your sign-in, saved passwords and extensions" aria-label="Open in browser" />
          <Key size="sm" variant="ghost" icon="x" onClick={onBack} aria-label="Close" />
        </>}>
        <div className="mpage-banner">
          {banner && <img src={banner} alt="" />}
          <div className="mpage-head">
            <h2>{c.name}</h2>
            <div className="m-meta"><span>by {c.user.name}</span>{c.category && <span>{c.category.name}</span>}{r && <span>revision {r.revisionNumber}</span>}</div>
            {c.summary && <p>{c.summary}</p>}
            <div className="actions">{r?.gameVersions.map((g) => <Pill key={g.reference}>Game {g.reference}</Pill>)}{(c.adultContent || r?.adultContent) && <Pill tone="over">Adult content</Pill>}
              <Key size="sm" variant="ghost" icon="web" onClick={() => onNexus(c.slug)} title="The collection's page on Nexus: comments, bugs, revisions and its curator">View on Nexus</Key></div>
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
            {shots.length > 0 && <MediaStage items={shots.map((s) => ({ ...s, caption: c.name }))} />}
            {links.length > 0 && <Links key={c.slug} list={links} />}
            <div className="sub-h">Description</div>
            {more === null ? <p className="dim">Loading the description</p> : <div className="desc">{desc ? <DescText parts={parts} /> : <span className="dim">No description.</span>}</div>}
            {more && !more.description && <p className="dim" style={{ fontSize: ".84em" }}>Only the summary is shown: Nexus did not give the full description here. Open the page for it.</p>}
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
  const [busy, setBusy] = useState(false);
  useEffect(() => { settings.get("flags.publishCollections").then((v) => setPublish(v === "1")).catch(() => {}); }, []);
  const run = async (f: () => Promise<void>) => { setBusy(true); try { await f(); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); } };

  return (
    <div className="grid2" style={{ alignItems: "start" }}>
      <Module icon="share" title="Make a collection" count="from the active profile">
        <p>Pins each enabled mod's Nexus mod ID, file ID and archive hash, and which mods need which. Share the file; others import it here.</p>
        <div className="stack" style={{ gap: ".5em", marginTop: ".6em" }}>
          <label className="fbox" style={{ flex: "none" }}><input className="finput" value={name} onChange={(e) => setName(e.target.value)} placeholder="Collection name" aria-label="Collection name" /></label>
          <label className="fbox" style={{ flex: "none" }}><input className="finput" value={desc} onChange={(e) => setDesc(e.target.value)} placeholder="What it is for (optional)" aria-label="Description" /></label>
          <div className="actions">
            <Key icon="collections" disabled={!name.trim() || busy} onClick={() => run(async () => setMade(await collections.createLocal(name, desc)))}>Make it</Key>
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
            <Key variant="primary" icon="share" disabled={busy} onClick={() => run(async () => {
              const path = await pickSavePath(`${made.name.replace(/[^\w -]/g, "_")}.pmcollection.json`);
              if (path) { await collections.exportLocal(made, path); toast("Collection saved."); }
            })}>Export</Key>
            <Key variant="ghost" disabled={!publish} title={publish ? "Experimental" : "Turn on the experiment below first"}
              onClick={() => toast("Publishing to Nexus is not available: Nexus has not confirmed that other apps may publish collections.", true)}>Publish to Nexus</Key>
          </div>
        </>}
        <div className="sub-h">Experiments</div>
        <Switch checked={publish} onChange={(v) => { setPublish(v); settings.set("flags.publishCollections", v ? "1" : "0").catch(() => {}); }}>Publishing collections to Nexus (unconfirmed, does nothing yet)</Switch>
      </Module>

      <Module icon="file" title="Import a collection" count={imported ? imported.c.name : undefined}>
        <p>Open a collection file someone shared. It is checked before anything installs: game version, what you already have, and whether each pinned file is still on Nexus.</p>
        <div className="actions" style={{ marginTop: ".6em" }}>
          <Key icon="file" disabled={busy} onClick={() => run(async () => {
            const path = await pickCollectionFile();
            if (path) { const [c, check] = await collections.importLocal(path); setImported({ c, check }); }
          })}>Open a file</Key>
        </div>
        {imported && <CheckDialog title={imported.c.name} load={() => Promise.resolve(imported.check)} onClose={() => setImported(null)} onQueued={() => { setImported(null); onQueued(); }} />}
        {!imported && <Empty icon="collections" title="Nothing imported"><Icon name="info" size={9} /> Pockymod collection files end in .pmcollection.json.</Empty>}
      </Module>
    </div>
  );
}
