// A mod's detail: picture, LCDs, console commands, the description's links, files, requirements, the indexed file list, description, and the
// screen's one orange key, Install (or, once it is installed, the menu of what the app can do to it). It shows in a
// drawer, or expanded as a page laid out like the mod's Nexus page: the banner, the media on a big stage, the
// description, and the files and requirements beside them.
import { useEffect, useState } from "react";
import { Icon } from "../../ui/Icon";
import { Empty, Key, Lcd, Module, Tag, Veil, bytesText, formatCount } from "../../ui/pocket";
import { useAdult } from "../../lib/prefs";
import {
  FRAMEWORKS, browse, descriptionMedia, endorseMod, errorText, fileBytes, nexusUrl, openExternal, trackMod,
  type CardView, type Classification, type DetailView, type InstallRow, type ModFile, type Update,
} from "../../lib/api";
import { descriptionLinks, descriptionParts, descriptionText, shortDate } from "../../lib/text";
import { extractCommands } from "../../lib/commands";
import { packCommandsFor, useExtensions } from "../../lib/extensions";
import { toast } from "../../ui/toast";
import { Gallery, MediaStage } from "./Media";
import { DescText, Links } from "./Links";
import { ModKey, mainFile } from "./ModActions";

export function TypeTag({ c }: { c: Classification }) {
  const label = c.modType === "redmod" ? "REDmod" : c.modType === "unknown" ? "Type unknown" : c.modType[0].toUpperCase() + c.modType.slice(1);
  return <Tag kind={c.modType === "unknown" || c.modType === "mixed" ? undefined : c.modType} square>{label}</Tag>;
}

export function FrameworkTags({ c }: { c: Classification }) {
  return <>{c.frameworks.map((f) => <Tag key={f.key} kind={f.key}>{FRAMEWORKS.find((x) => x.key === f.key)?.short ?? f.name}</Tag>)}</>;
}

const CURRENT = ["MAIN", "OPTIONAL", "UPDATE", "MISCELLANEOUS"];

export interface DetailProps {
  card: CardView; onFlag: (c: CardView, f: "hidden" | "ignored", on: boolean) => void;
  onInstall?: (d: DetailView, fileId: number) => void; onOpenPage?: (modId: number) => void; onOpen: (modId: number) => void; onClose?: () => void;
  /** Shown as a page (true) or in the drawer; onWide switches. */
  wide?: boolean; onWide?: (wide: boolean) => void;
  /** The user's copies of this mod, the updates and the queue, for the installed menu. */
  installed?: InstallRow[]; updates?: Update[]; queued?: string; onLibrary?: () => void;
}

export function Detail(p: DetailProps) {
  const { card } = p;
  const [d, setD] = useState<DetailView | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [fileId, setFileId] = useState<number | null>(null);

  useEffect(() => {
    let live = true;
    setD(null); setErr(null);
    browse.detail(card.modId, card.updatedAt || undefined).then((v) => {
      if (!live) return;
      setD(v);
      setFileId(mainFile(v.files)?.fileId ?? null);
    }).catch((e) => live && setErr(errorText(e)));
    return () => { live = false; };
  }, [card.modId, card.updatedAt]);

  const title = d?.detail.name || card.name || "Mod";
  if (err) return <Module icon="warn" title={title} band="bad" fill tools={<HeadKeys p={p} />}><div className="note bad">{err}</div></Module>;
  if (!d) return <Module icon="info" title={title} count="loading" fill tools={<HeadKeys p={p} />}><Empty icon="clock" title="Loading" /></Module>;
  return p.wide ? <Page p={p} d={d} setD={setD} fileId={fileId} setFileId={setFileId} /> : <Drawer p={p} d={d} setD={setD} fileId={fileId} setFileId={setFileId} />;
}

type Parts = { p: DetailProps; d: DetailView; setD: (d: DetailView) => void; fileId: number | null; setFileId: (n: number) => void };

/** The keys in the band: endorse, track, the Nexus page (here or in your browser), expand or back, close. */
function HeadKeys({ p, d }: { p: DetailProps; d?: DetailView }) {
  const m = d?.detail;
  return <>
    {m && <Key size="sm" variant="ghost" icon="star" onClick={() => endorseMod(m.modId, m.version).then(() => toast(`Endorsed ${m.name}`)).catch((e) => toast(errorText(e), true))} title="Endorse on Nexus" aria-label="Endorse" />}
    {m && <Key size="sm" variant="ghost" icon="eye" onClick={() => trackMod(m.modId).then(() => toast(`Tracking ${m.name}`)).catch((e) => toast(errorText(e), true))} title="Track on Nexus" aria-label="Track" />}
    <Key size="sm" variant="ghost" icon="web" onClick={() => p.onOpenPage?.(p.card.modId)} title="Nexus page" aria-label="Open the Nexus page" />
    <Key size="sm" variant="ghost" icon="link" onClick={() => openExternal(nexusUrl.mod(p.card.modId)).catch((e) => toast(errorText(e), true))} title="Open in your browser" aria-label="Open in browser" />
    {p.onWide && (p.wide
      ? <Key size="sm" variant="ghost" icon="chevron" className="flip" onClick={() => p.onWide?.(false)} title="Show in the side drawer">Drawer</Key>
      : <Key size="sm" variant="ghost" icon="max" onClick={() => p.onWide?.(true)} title="Open as a full page">Expand</Key>)}
    {p.onClose && <Key size="sm" variant="ghost" icon="x" onClick={p.onClose} aria-label="Close" />}
  </>;
}

function useParts({ p, d }: Parts) {
  const packs = useExtensions().active.commandPacks;
  const m = d.detail;
  const current = d.files.filter((f) => CURRENT.includes(f.category)).sort((a, b) => b.date - a.date);
  const parts = descriptionParts(m.description);
  const desc = descriptionText(m.description);
  const links = descriptionLinks(parts);
  // the description's own commands, then any from extension command packs that list this mod
  const fromPacks = packCommandsFor(packs, m.modId);
  const own = extractCommands(desc);
  const commands = [...own, ...fromPacks.filter((x) => !own.some((c) => c.command === x.command))];
  const shots = descriptionMedia(m.description, m.pictureUrl);
  const view: CardView = { ...p.card, hidden: d.hidden, ignored: d.ignored };
  const [shown, setShown] = useState<number | null>(null);
  const blur = useAdult() === "blur" && !!m.adultContent && shown !== m.modId;
  const veil = { on: blur, onShow: () => setShown(m.modId) };
  return { m, current, desc, parts, links, commands, shots, view, veil };
}

/** Install, or the installed menu; then hide and ignore. */
function Actions({ p, d, setD, file, view }: { p: DetailProps; d: DetailView; setD: (d: DetailView) => void; file?: ModFile; view: CardView }) {
  const m = d.detail;
  const have = p.installed ?? [];
  return (
    <div className="actions">
      {have.length === 0
        ? <Key variant="primary" size="lg" icon="install" disabled={!file || !!p.queued} onClick={() => file && p.onInstall?.(d, file.fileId)}>{p.queued ?? "Install"}</Key>
        : <>
          <ModKey mod={{ modId: m.modId, name: m.name, version: m.version }} rows={have} updates={p.updates} size="lg" onLibrary={p.onLibrary}
            onFlag={(f, on) => { p.onFlag(view, f, on); setD({ ...d, [f]: on }); }} flags={{ hidden: d.hidden, ignored: d.ignored }} />
          {file && !have.some((r) => r.fileId === file.fileId) && !p.updates?.some((u) => u.modId === m.modId && u.fileId === file.fileId) &&
            <Key icon="install" onClick={() => p.onInstall?.(d, file.fileId)} title="Install the selected file too">Install this file</Key>}
        </>}
      <Key variant="ghost" icon="web" onClick={() => p.onOpenPage ? p.onOpenPage(m.modId) : void openExternal(nexusUrl.mod(m.modId)).catch((e) => toast(errorText(e), true))} title="Posts, bugs and articles">View on Nexus</Key>
      <Key variant="ghost" icon="hide" onClick={() => { p.onFlag(view, "hidden", !d.hidden); setD({ ...d, hidden: !d.hidden }); }}>{d.hidden ? "Unhide" : "Hide"}</Key>
      <Key variant="ghost" icon="x" onClick={() => { p.onFlag(view, "ignored", !d.ignored); setD({ ...d, ignored: !d.ignored }); }}>{d.ignored ? "Stop ignoring" : "Ignore"}</Key>
    </div>
  );
}

function FileRows({ files, fileId, setFileId, have, limit = 8 }: { files: ModFile[]; fileId: number | null; setFileId: (n: number) => void; have: InstallRow[]; limit?: number }) {
  if (files.length === 0) return <p className="dim">No current files</p>;
  return (
    <div className="rows">
      {files.slice(0, limit).map((f) => (
        <button key={f.fileId} type="button" className={`row${f.fileId === fileId ? " sel" : ""}`} onClick={() => setFileId(f.fileId)}>
          <span className="m-main"><span className="m-t">{f.name}</span><span className="m-meta"><span>{f.category.toLowerCase().replace("_", " ")}</span><span>v{f.version}</span><span>{shortDate(new Date(f.date * 1000).toISOString())}</span>{have.some((r) => r.fileId === f.fileId) && <span className="okc">installed</span>}</span></span>
          <span className="m-side"><span>{bytesText(fileBytes(f))}</span></span>
        </button>
      ))}
    </div>
  );
}

function Requires({ d, onOpen }: { d: DetailView; onOpen: (id: number) => void }) {
  const r = d.detail.modRequirements.nexusRequirements;
  if (r.totalCount === 0) return null;
  return <>
    <div className="sub-h">Requires</div>
    <div className="rows">
      {r.nodes.map((x) => (
        <button key={`${x.modId}-${x.modName}`} type="button" className="row" onClick={() => !x.externalRequirement && onOpen(x.modId)} disabled={x.externalRequirement}>
          <span className="m-main"><span className="m-t">{x.modName}</span>{x.notes && <span className="m-meta"><span>{x.notes}</span></span>}</span>
          <span className="m-side">{x.externalRequirement ? <span>Off Nexus</span> : <Icon name="chevron" size={9} />}</span>
        </button>
      ))}
    </div>
  </>;
}

function ArchiveList({ d }: { d: DetailView }) {
  const [show, setShow] = useState(false);
  return <>
    <div className="sub-h">Files in the archive</div>
    {d.fileList ? (
      <>
        <p className="dim" style={{ fontSize: ".86em" }}>{d.fileList.length} paths{" "}
          <button type="button" className="linkish" onClick={() => setShow(!show)}>{show ? "Hide" : "Show"}</button></p>
        {show && <div className="log" style={{ maxHeight: "12em" }}>{d.fileList.slice(0, 200).join("\n")}</div>}
      </>
    ) : <p className="dim" style={{ fontSize: ".86em" }}>No file list from Nexus yet</p>}
  </>;
}

function Drawer(x: Parts) {
  const { p, d, setD, fileId, setFileId } = x;
  const { m, current, desc, parts, links, commands, shots, view, veil } = useParts(x);
  const [full, setFull] = useState(false);
  const file = d.files.find((f) => f.fileId === fileId);
  return (
    <Module icon="library" title={m.name} count={`v${m.version}`} fill className="detail-mod" tools={<HeadKeys p={p} d={d} />}>
      <Veil {...veil}>{shots.length > 0 ? <Gallery items={shots} /> : m.pictureUrl && <img className="hero-img" src={m.pictureUrl} alt="" />}</Veil>
      <div className="m-meta" style={{ marginTop: ".6em" }}><span>{m.author ?? m.uploader?.name}</span><span>{m.category}</span><span>Updated {shortDate(m.updatedAt)}</span></div>
      <p style={{ margin: ".5em 0 .2em", color: "var(--text)" }}>{m.summary}</p>
      <div className="m-tags"><TypeTag c={d.classification} /><FrameworkTags c={d.classification} /></div>
      {commands.length > 0 && <Commands list={commands} />}
      {links.length > 0 && <Links key={m.modId} list={links} onMod={p.onOpen} />}

      <div className="lcds" style={{ marginTop: ".8em" }}>
        <Lcd size="sm" label="Downloads" value={formatCount(m.downloads)} />
        <Lcd size="sm" label="Endorsed" value={formatCount(m.endorsements)} />
        {file && <Lcd size="sm" label="Size" value={bytesText(fileBytes(file)).split(" ")[0]} unit={bytesText(fileBytes(file)).split(" ")[1]} />}
      </div>

      <div className="sub-h">File</div>
      <FileRows files={current} fileId={fileId} setFileId={setFileId} have={p.installed ?? []} />
      <div style={{ marginTop: ".8em" }}><Actions p={p} d={d} setD={setD} file={file} view={view} /></div>

      <Requires d={d} onOpen={p.onOpen} />
      <ArchiveList d={d} />

      <div className="sub-h">Description</div>
      <Veil {...veil}><div className="desc"><DescText parts={parts} limit={full ? undefined : 900} onMod={p.onOpen} />{desc.length > 900 && !full && "…"}</div></Veil>
      {desc.length > 900 && <Key size="sm" variant="ghost" onClick={() => setFull(!full)}>{full ? "Less" : "Read more"}</Key>}
    </Module>
  );
}

/** The expanded page: the banner across the top, then the media and description beside the files. */
function Page(x: Parts) {
  const { p, d, setD, fileId, setFileId } = x;
  const { m, current, desc, parts, links, commands, shots, view, veil } = useParts(x);
  const file = d.files.find((f) => f.fileId === fileId);
  const banner = m.pictureUrl ?? shots.find((s) => s.kind === "image")?.url ?? null;
  const archived = d.files.filter((f) => !CURRENT.includes(f.category)).length;
  return (
    <Module icon="library" title={m.name} count={`v${m.version}`} className="mpage" tools={<HeadKeys p={p} d={d} />}>
      <div className="mpage-banner">
        {banner && <img src={banner} alt="" className={veil.on ? "veiled-img" : undefined} />}
        <div className="mpage-head">
          <h2>{m.name}</h2>
          <div className="m-meta"><span>by {m.author ?? m.uploader?.name ?? "unknown"}</span><span>{m.category}</span><span>v{m.version}</span><span>Updated {shortDate(m.updatedAt)}</span>{m.createdAt && <span>Added {shortDate(m.createdAt)}</span>}</div>
          {m.summary && <p>{m.summary}</p>}
          <div className="m-tags"><TypeTag c={d.classification} /><FrameworkTags c={d.classification} /></div>
        </div>
      </div>
      <div className="mpage-body">
        <div className="mpage-main">
          {shots.length > 0 && <Veil {...veil}><MediaStage items={shots} /></Veil>}
          {commands.length > 0 && <Commands list={commands} />}
          {links.length > 0 && <Links key={m.modId} list={links} onMod={p.onOpen} />}
          <div className="sub-h">Description</div>
          <Veil {...veil}><div className="desc">{desc ? <DescText parts={parts} onMod={p.onOpen} /> : <span className="dim">No description</span>}</div></Veil>
        </div>
        <aside className="mpage-side">
          <div className="lcds">
            <Lcd size="sm" label="Downloads" value={formatCount(m.downloads)} />
            <Lcd size="sm" label="Endorsed" value={formatCount(m.endorsements)} />
            {file && <Lcd size="sm" label="Size" value={bytesText(fileBytes(file)).split(" ")[0]} unit={bytesText(fileBytes(file)).split(" ")[1]} />}
          </div>
          <Actions p={p} d={d} setD={setD} file={file} view={view} />
          <div className="sub-h">Files {archived > 0 && <span className="dim">· {archived} older on Nexus</span>}</div>
          <FileRows files={current} fileId={fileId} setFileId={setFileId} have={p.installed ?? []} limit={30} />
          {file?.description && <p className="dim" style={{ fontSize: ".86em", marginTop: ".4em" }}><DescText parts={descriptionParts(file.description)} onMod={p.onOpen} /></p>}
          <Requires d={d} onOpen={p.onOpen} />
          <ArchiveList d={d} />
        </aside>
      </div>
    </Module>
  );
}

/** Console commands from the description, at the top of the detail in a dropdown that opens. */
export function Commands({ list }: { list: Array<{ command: string; generated: boolean; from?: string; label?: string }> }) {
  const [open, setOpen] = useState(list.length <= 3);
  const copy = (t: string) => navigator.clipboard.writeText(t).then(() => toast("Copied")).catch(() => toast("Couldn't copy", true));
  return (
    <div className="cmds">
      <button type="button" className="cmds-h" aria-expanded={open} onClick={() => setOpen(!open)}>
        <Icon name="chevron" size={9} className={open ? "rot" : ""} />
        <b>Console commands</b><span className="n">{list.length}</span>
        <span className="sp" />
        <span className="dim">Cyber Engine Tweaks console</span>
      </button>
      {open && <>
        <div className="cmds-list">
          {list.map((c) => (
            <div key={c.command} className="cmd">
              <code>{c.command}</code>
              {c.generated && <span className="dim" title="Made from an item ID in the description">from item ID</span>}
              {c.from && <span className="dim from" title={`From ${c.from}`}>{c.label ?? c.from}</span>}
              <Key size="sm" variant="ghost" icon="file" aria-label={`Copy ${c.command}`} title="Copy" onClick={() => copy(c.command)} />
            </div>
          ))}
        </div>
        {list.length > 1 && <Key size="sm" icon="file" onClick={() => copy(list.map((c) => c.command).join("\n"))}>Copy all</Key>}
      </>}
    </div>
  );
}
