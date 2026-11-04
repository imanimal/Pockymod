// Collections: browse Nexus collections and check one before installing; make, export and import local ones.
import { useEffect, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { Chip, Empty, Key, Lcd, Module, Pill, SearchField, Seg, Switch, bytesText, formatCount } from "../ui/pocket";
import {
  collections, errorText, pickCollectionFile, pickSavePath, settings,
  type CollectionCard, type CollectionCheck, type CollectionsPage, type LocalCollection,
} from "../lib/api";
import { toast } from "../ui/toast";

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
  useEffect(() => { const t = setTimeout(() => setQ((o) => ({ ...o, text, offset: 0 })), 350); return () => clearTimeout(t); }, [text]);
  useEffect(() => { collections.browse(q).then((p) => { setPage(p); setErr(null); }).catch((e) => setErr(errorText(e))); }, [q]);
  const versions = Object.entries(page?.facetsData?.gameVersion ?? {}).filter(([v]) => v).sort((a, b) => b[1] - a[1]).slice(0, 8);

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
            <Module key={c.id} icon="collections" title={c.name} count={r ? `${r.modCount} mods` : undefined}
              tools={<Key size="sm" variant="ghost" onClick={() => setOpen(c)}>Check</Key>}>
              {c.tileImage && <img className="hero-img" src={c.tileImage.thumbnailUrl ?? c.tileImage.url} alt="" loading="lazy" />}
              <b className="hi" style={{ display: "block", marginTop: ".5em" }}>{c.name}</b>
              <div className="m-meta"><span>{c.user.name}</span>{c.category && <span>{c.category.name}</span>}</div>
              <p style={{ fontSize: ".88em", margin: ".3em 0", display: "-webkit-box", WebkitLineClamp: 2, WebkitBoxOrient: "vertical", overflow: "hidden" }}>{c.summary}</p>
              <div className="actions">
                {r?.gameVersions.map((g) => <Pill key={g.reference}>{g.reference}</Pill>)}
                <span className="mono dim" style={{ fontSize: ".78em" }}>★ {formatCount(c.endorsements)} · ↓ {formatCount(c.totalDownloads)} · {r ? bytesText(r.totalSize) : ""}</span>
              </div>
            </Module>
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

/** Pre-install checks, then Install. */
function CheckDialog({ title, load, onClose, onQueued }: { title: string; load: () => Promise<CollectionCheck>; onClose: () => void; onQueued: () => void }) {
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
    try { await collections.install(c, optional); toast(`${c.name} is queued: ${c.missing} mods.`); onQueued(); } catch (e) { toast(errorText(e), true); setBusy(false); }
  };
  const shown = c?.entries.filter((e) => e.status !== "installed") ?? [];
  return (
    <Dialog icon="collections" title={title} onClose={onClose}>
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
        {c.needsClick > 20 && <div className="note warn">That is {c.needsClick} clicks on Nexus. Each page opens here in turn, and the queue moves on as soon as you click.</div>}
        <div className="rows" style={{ maxHeight: "16em", overflow: "auto" }}>
          {shown.slice(0, 80).map((e) => (
            <div key={`${e.modId}-${e.fileId}`} className={`row${e.status === "unavailable" && !e.replacement ? " off" : ""}`}>
              <span className="m-main"><span className="m-t">{e.name}</span>
                <span className="m-meta"><span>{e.fileName}</span><span>v{e.version}</span>{e.optional && <span>optional</span>}
                  {e.replacement && <span>file gone: using v{e.replacement[1]}</span>}</span></span>
              <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".4em" }}>
                <span>{bytesText(e.size)}</span>
                {e.status === "other" && <Pill tone="soon">Other version installed</Pill>}
                {e.status === "unavailable" && <Pill tone={e.replacement ? "soon" : "over"}>{e.replacement ? "Replaced" : "Unavailable"}</Pill>}
              </span>
            </div>
          ))}
          {shown.length > 80 && <p className="dim" style={{ padding: ".5em" }}>and {shown.length - 80} more</p>}
        </div>
        <p className="dim" style={{ fontSize: ".82em" }}>Installer choices and rules saved in the collection are not applied yet; each mod installs with its default layout.</p>
        <div className="actions" style={{ justifyContent: "flex-end" }}>
          {c.entries.some((e) => e.optional) && <Switch checked={optional} onChange={setOptional}>Include optional mods</Switch>}
          <Key variant="ghost" onClick={onClose}>Cancel</Key>
          <Key variant="primary" icon="install" disabled={busy || c.missing === 0} onClick={install}>{c.missing ? `Install ${c.missing} mods` : "All installed"}</Key>
        </div>
      </>}
    </Dialog>
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
          <div className="sub-h">{made.name} · {made.mods.length} mods · game {made.gameVersion ?? "?"}</div>
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
        {!imported && <Empty icon="collections" title="Nothing imported"><Icon name="info" size={9} /> Pocket Mods collection files end in .pmcollection.json.</Empty>}
      </Module>
    </div>
  );
}
