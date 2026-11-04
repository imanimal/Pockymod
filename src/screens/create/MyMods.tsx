// My mods: the member's Cyberpunk mods as cards (picture, status, numbers, current files with their ids), LCD totals,
// and a manage drawer per mod with the website pages, share links, every file version, the changelog and both
// requirement lists. Read-only without a key.
import { useEffect, useState } from "react";
import { Icon } from "../../ui/Icon";
import { Empty, Key, Lcd, Module, Pill, Seg, bytesText, formatCount } from "../../ui/pocket";
import { authoring, errorText, fileLink, nexusPages, type AuthorFile, type ModManage, type MyMod, type MyMods as MyModsData, type NexusUser } from "../../lib/api";
import { shortDate } from "../../lib/text";
import { toast } from "../../ui/toast";
import { ScanPill, StatusPill, categoryOf, copy, fileCategoryText, openNexus, unixDate, type PublishPrefill } from "./shared";

export function prefillFor(m: { modId: number; name: string }, f: AuthorFile): PublishPrefill {
  return { modId: m.modId, modName: m.name, fileId: f.groupId != null ? String(f.groupId) : "", fileName: f.name, category: categoryOf(f.category), currentVersion: f.version };
}

const bytesOf = (f: AuthorFile) => f.sizeInBytes ?? f.size * 1024;
const openPage = (modId: number) => nexusPages.mod(modId).catch((e) => toast(errorText(e), true));

export function MyMods({ mine, loading, error, me, who, onLoad, onPublish, onNew, onChanged }: {
  mine: MyModsData | null; loading: boolean; error: string | null; me: NexusUser | null; who: string;
  onLoad: (user: string, refresh?: boolean) => void; onPublish: (p: PublishPrefill | null) => void; onNew: () => void; onChanged: () => void;
}) {
  const [text, setText] = useState(who);
  const [other, setOther] = useState(false);
  const [open, setOpen] = useState<MyMod | null>(null);
  const [pin, setPin] = useState("");
  useEffect(() => setText(who), [who]);

  const lookup = (
    <form className="cr-who" onSubmit={(e) => { e.preventDefault(); onLoad(text.trim()); }}>
      <label className="fbox">
        <Icon name="user" />
        <input className="finput" value={text} onChange={(e) => setText(e.target.value)} placeholder="Nexus user name, member ID or profile link" aria-label="Nexus user name or member ID" />
      </label>
      <Key type="submit" icon="browse" disabled={!text.trim() || loading}>Show mods</Key>
      {me && <Key variant="ghost" onClick={() => { setOther(false); setText(""); onLoad(""); }}>Back to mine</Key>}
    </form>
  );

  if (!mine) return (
    <Module icon="user" title="My mods" count={loading ? "loading" : undefined}>
      {loading ? <Empty icon="clock" title="Asking Nexus for your mods" /> : <>
        <p>See the mods you uploaded for Cyberpunk 2077: their status, numbers and files, and publish new versions without hunting for IDs.</p>
        <div style={{ marginTop: ".7em" }}>{lookup}</div>
        {error && <div className="note warn" style={{ marginTop: ".6em" }}>{error}</div>}
        <p className="dim hint">Without an API key this is read-only and shows published mods. Add your key in Settings to publish versions from here.</p>
      </>}
    </Module>
  );

  const a = mine.author;
  const t = mine.totals;
  const month = mine.thisMonth;
  const addPin = async () => {
    try { await authoring.pin(a.memberId, pin); setPin(""); toast("Added. It shows here once Nexus lists it."); onChanged(); } catch (e) { toast(errorText(e), true); }
  };

  return (
    <div className="stack">
      <Module icon="user" title={mine.own ? "My mods" : "Mods by"} count={a.name}
        tools={<>
          <Key size="sm" variant="ghost" icon="refresh" disabled={loading} onClick={() => onLoad(who, true)} aria-label="Refresh from Nexus" title="Refresh from Nexus" />
          <Key size="sm" variant="ghost" icon="web" onClick={() => openNexus(mine.links.profile)} title="Profile on Nexus" aria-label="Profile on Nexus" />
        </>}>
        <div className="cr-head">
          {a.avatar && <img className="cr-avatar" src={a.avatar} alt="" />}
          <div className="m-main">
            <span className="m-t">{a.name}{a.recognizedAuthor && <> <Pill tone="ok">Recognised author</Pill></>}</span>
            <span className="m-meta">
              <span>{mine.own ? "Signed in" : me ? "Another member, read-only" : "Read-only, published mods"}</span>
              {a.joined && <span>Member since {shortDate(a.joined)}</span>}
              <span>{loading ? "Refreshing" : `Checked ${new Date(mine.fetchedAt * 1000).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" })}`}</span>
            </span>
          </div>
        </div>
        <div className="lcds" style={{ marginTop: ".7em" }}>
          <Lcd label="Mods" value={String(t.mods).padStart(2, "0")} sub={a.modCount > t.mods ? `${a.modCount} in all games` : "Cyberpunk 2077"} />
          <Lcd label="Downloads" value={formatCount(t.downloads)} sub={t.uniqueDownloads ? `${formatCount(t.uniqueDownloads)} unique` : undefined} />
          <Lcd label="Endorsements" value={formatCount(t.endorsements)} />
          {month && <Lcd label={`This month`} value={formatCount(month.downloads)} sub={`downloads, all games`} />}
        </div>
        <div className="actions" style={{ marginTop: ".7em" }}>
          <Key variant="primary" icon="share" disabled={mine.mods.length === 0} onClick={() => onPublish(null)}>Publish a version</Key>
          <Key icon="plus" onClick={onNew}>Upload a new mod</Key>
          <span className="sp" style={{ flex: 1 }} />
          {!other ? <Key variant="ghost" size="sm" icon="browse" onClick={() => setOther(true)}>Look up another member</Key> : null}
        </div>
        {other && <div style={{ marginTop: ".6em" }}>{lookup}</div>}
        {!mine.own && <p className="dim hint">{me ? "These are another member's mods, so publishing is off for them." : "Read-only: Nexus shows published mods without a key. Add your API key in Settings to see hidden ones (if Nexus lists them) and to publish."}</p>}
        {error && <div className="note warn" style={{ marginTop: ".6em" }}>{error}</div>}
      </Module>

      {mine.mods.length === 0 ? (
        <Module icon="library" title="No mods yet" count="Cyberpunk 2077">
          <Empty icon="share" title={`${a.name} has no published Cyberpunk 2077 mods`} actions={<Key icon="plus" onClick={onNew}>Upload a new mod</Key>}>
            A new or hidden mod shows once Nexus lists it. You can add it by its link below.
          </Empty>
        </Module>
      ) : (
        <div className="cr-grid">
          {mine.mods.map((m) => <ModCard key={m.modId} m={m} canPublish={mine.own} onPublish={onPublish} onManage={() => setOpen(m)} />)}
        </div>
      )}

      <Module icon="link" title="Add a mod by its link" count={mine.missingPinned.length ? `${mine.missingPinned.length} not listed yet` : undefined}>
        <p className="dim" style={{ fontSize: ".9em" }}>A page you just made, or a mod you work on in someone else's team. Paste the link to its page or its number.</p>
        <form className="cr-who" style={{ marginTop: ".5em" }} onSubmit={(e) => { e.preventDefault(); void addPin(); }}>
          <label className="fbox"><Icon name="link" /><input className="finput" value={pin} onChange={(e) => setPin(e.target.value)} placeholder="nexusmods.com/cyberpunk2077/mods/…" aria-label="Mod link or number" /></label>
          <Key type="submit" icon="plus" disabled={!pin.trim()}>Add</Key>
        </form>
        {mine.missingPinned.map((x) => (
          <div key={x.modId} className="note cr-missing">
            <span>Mod {x.modId} is not listed yet. Nexus shows a mod to apps once it is published and its file is scanned.</span>
            <Key size="sm" variant="ghost" icon="web" onClick={() => openNexus(x.links.edit)}>Edit page</Key>
            <Key size="sm" variant="ghost" icon="x" onClick={() => authoring.unpin(a.memberId, x.modId).then(onChanged).catch((e) => toast(errorText(e), true))}>Remove</Key>
          </div>
        ))}
      </Module>

      {open && <Manage m={open} canPublish={mine.own} onPublish={(p) => { setOpen(null); onPublish(p); }} onClose={() => setOpen(null)}
        onUnpin={open.pinned ? () => authoring.unpin(a.memberId, open.modId).then(() => { setOpen(null); onChanged(); }).catch((e) => toast(errorText(e), true)) : undefined} />}
    </div>
  );
}

function ModCard({ m, canPublish, onPublish, onManage }: { m: MyMod; canPublish: boolean; onPublish: (p: PublishPrefill) => void; onManage: () => void }) {
  return (
    <section className="panel cr-card">
      <header className="ph">
        <span className="pt"><Icon name="library" />{m.name}</span>
        <span className="cnt">v{m.version}</span>
        <span className="sp" />
        {m.pinned && <Pill>Added by you</Pill>}
        <StatusPill status={m.status} />
      </header>
      <div className="pb">
        <div className="cr-top">
          <button type="button" className="cr-pic" onClick={onManage} aria-label={`Manage ${m.name}`}>
            {m.thumbnailUrl || m.pictureUrl ? <img src={m.thumbnailUrl ?? m.pictureUrl ?? ""} alt="" loading="lazy" /> : <Icon name="library" size={36} />}
          </button>
          <div className="cr-info">
            <span className="m-meta"><span>{m.category || "No category"}</span><span>Updated {shortDate(m.updatedAt)}</span>{m.adultContent && <span>Adult</span>}</span>
            <div className="lcds cr-lcds">
              <Lcd size="sm" label="Downloads" value={formatCount(m.downloads)} />
              <Lcd size="sm" label="Unique" value={m.uniqueDownloads == null ? "--" : formatCount(m.uniqueDownloads)} />
              <Lcd size="sm" label="Endorsed" value={formatCount(m.endorsements)} />
            </div>
          </div>
        </div>
        <div className="sub-h">Files <span className="dim mono">{m.files.length}{m.oldFiles ? ` · ${m.oldFiles} older` : ""}</span></div>
        {m.files.length === 0 ? <p className="dim" style={{ fontSize: ".86em" }}>No current files. Add one on the website's Files step.</p> : (
          <div className="rows">
            {m.files.slice(0, 5).map((f) => <FileRow key={f.fileId} f={f} onPublish={canPublish ? () => onPublish(prefillFor(m, f)) : undefined} />)}
            {m.files.length > 5 && <button type="button" className="linkish" style={{ margin: ".3em .6em", fontSize: ".86em" }} onClick={onManage}>{m.files.length - 5} more files</button>}
          </div>
        )}
        <div className="m-meta" style={{ marginTop: ".5em" }}>
          <span>Requires {m.requirements}</span><span>Required by {formatCount(m.requiredBy)}</span>
        </div>
        <div className="actions cr-foot">
          <Key size="sm" icon="settings" onClick={onManage}>Manage</Key>
          <Key size="sm" variant="ghost" icon="web" onClick={() => openNexus(m.links.edit)}>Edit on Nexus</Key>
          <Key size="sm" variant="ghost" icon="link" onClick={() => copy(m.links.page, "Link copied.")}>Copy link</Key>
        </div>
      </div>
    </section>
  );
}

function FileRow({ f, onPublish, onCopy }: { f: AuthorFile; onPublish?: () => void; onCopy?: () => void }) {
  return (
    <div className="row cr-file">
      <span className="m-main">
        <span className="m-t">{f.name}</span>
        <span className="m-meta">
          <span>{fileCategoryText(f.category)}</span><span>v{f.version}</span><span>{bytesText(bytesOf(f))}</span>
          <span className="mono" title="The file's id on Nexus">ID {f.fileId}</span>
        </span>
      </span>
      <span className="cr-file-k">
        <ScanPill scan={f.scannedV2} />
        {onCopy && <Key size="sm" variant="ghost" icon="link" onClick={onCopy} aria-label={`Copy a link to ${f.name}`} title="Copy a link to this file" />}
        {onPublish && <Key size="sm" variant="dark" icon="share" onClick={onPublish} title="Publish a new version of this file">New version</Key>}
      </span>
    </div>
  );
}

function Manage({ m, canPublish, onPublish, onClose, onUnpin }: { m: MyMod; canPublish: boolean; onPublish: (p: PublishPrefill) => void; onClose: () => void; onUnpin?: () => void }) {
  const [d, setD] = useState<ModManage | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [files, setFiles] = useState<"current" | "all">("current");
  const [allBy, setAllBy] = useState(false);
  useEffect(() => {
    let live = true;
    authoring.modFiles(m.modId).then((v) => live && setD(v)).catch((e) => live && setErr(errorText(e)));
    return () => { live = false; };
  }, [m.modId]);
  useEffect(() => {
    const k = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onClose]);

  const list = d ? (files === "current" ? d.files.filter((f) => ["MAIN", "UPDATE", "OPTIONAL", "MISCELLANEOUS"].includes(f.category)) : d.files) : m.files;
  const main = m.files.find((f) => f.category === "MAIN") ?? m.files[0];
  const links = m.links;

  return (
    <div className="drawer-ov" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="drawer" role="dialog" aria-modal="true" aria-label={`Manage ${m.name}`}>
        <Module icon="settings" title={m.name} count={`v${m.version}`} fill className="cr-drawer"
          tools={<Key size="sm" variant="ghost" icon="x" onClick={onClose} aria-label="Close" />}>
          {m.pictureUrl && <img className="hero-img" src={m.pictureUrl} alt="" />}
          <div className="actions" style={{ marginTop: ".6em" }}><StatusPill status={m.status} /><span className="dim" style={{ fontSize: ".86em" }}>Created {shortDate(m.createdAt)} · updated {shortDate(m.updatedAt)}</span></div>
          <div className="lcds" style={{ marginTop: ".6em" }}>
            <Lcd size="sm" label="Downloads" value={formatCount(m.downloads)} />
            <Lcd size="sm" label="Unique" value={(d?.uniqueDownloads ?? m.uniqueDownloads) == null ? "--" : formatCount((d?.uniqueDownloads ?? m.uniqueDownloads)!)} />
            <Lcd size="sm" label="Endorsed" value={formatCount(m.endorsements)} />
            <Lcd size="sm" label="Required by" value={formatCount(d?.requiredByTotal ?? m.requiredBy)} />
          </div>

          <div className="actions" style={{ marginTop: ".8em" }}>
            {canPublish && main && <Key variant="primary" icon="share" onClick={() => onPublish(prefillFor(m, main))}>Publish a new version</Key>}
            <Key icon="web" onClick={() => openNexus(links.edit)}>Edit the page</Key>
            <Key icon="file" onClick={() => openNexus(links.editFiles)}>Manage files</Key>
          </div>
          {!canPublish && <p className="dim hint">Publishing is for the mod's owner, signed in with their API key.</p>}

          <div className="sub-h">On Nexus</div>
          <div className="actions">
            <Key size="sm" variant="ghost" icon="web" onClick={() => openNexus(links.page)}>Mod page</Key>
            <Key size="sm" variant="ghost" icon="file" onClick={() => openNexus(links.files)}>Files tab</Key>
            <Key size="sm" variant="ghost" icon="info" onClick={() => openNexus(links.posts)}>Posts</Key>
            <Key size="sm" variant="ghost" icon="warn" onClick={() => openNexus(links.bugs)}>Bugs</Key>
            <Key size="sm" variant="ghost" icon="clock" onClick={() => openNexus(links.logs)}>Logs</Key>
          </div>
          <p className="dim hint">Comments and bug reports open on the website: Nexus does not give apps a mod's posts.</p>

          <div className="sub-h">Share</div>
          <div className="actions">
            <Key size="sm" icon="link" onClick={() => copy(links.page, "Link copied.")}>Copy the link</Key>
            <Key size="sm" variant="ghost" icon="file" onClick={() => copy(links.files, "Link to the files copied.")}>Link to the files</Key>
            <Key size="sm" variant="ghost" icon="chip" onClick={() => copy(`[url=${links.page}]${m.name}[/url]`, "BBCode copied.")}>BBCode</Key>
            <Key size="sm" variant="ghost" icon="chip" onClick={() => copy(`[${m.name}](${links.page})`, "Markdown copied.")}>Markdown</Key>
          </div>

          <div className="sub-h cr-subh">
            <span>Files</span><span className="dim mono">{d ? `${d.files.length} versions` : ""}</span><span className="sp" style={{ flex: 1 }} />
            <Seg label="Which files" value={files} onChange={setFiles} options={[{ value: "current", label: "Current" }, { value: "all", label: "All versions" }]} />
          </div>
          {err && <div className="note bad">{err}</div>}
          <div className="rows">
            {list.slice(0, 60).map((f) => (
              <div key={f.fileId}>
                <FileRow f={f} onCopy={() => copy(fileLink(links, f.fileId), "Link to the file copied.")}
                  onPublish={canPublish && ["MAIN", "UPDATE", "OPTIONAL", "MISCELLANEOUS"].includes(f.category) ? () => onPublish(prefillFor(m, f)) : undefined} />
                {files === "all" && <div className="cr-file-sub dim">{shortDate(unixDate(f.date))} · {formatCount(f.totalDownloads)} downloads, {formatCount(f.uniqueDownloads)} unique{f.groupId != null && <> · update chain <span className="mono">{f.groupId}</span></>}</div>}
              </div>
            ))}
            {list.length === 0 && <p className="dim">No files.</p>}
          </div>

          <div className="sub-h">Changelog</div>
          {!d ? <p className="dim">Loading</p> : d.changelog.length === 0 ? <p className="dim" style={{ fontSize: ".9em" }}>No changelog on Nexus yet. Add one when you publish a version.</p> : (
            <div className="cr-log">
              {d.changelog.map((c) => (
                <div key={c.version} className="cr-log-v">
                  <div><b className="hi">{c.version}</b> <span className="dim">{shortDate(unixDate(c.date))}</span></div>
                  <ul>{c.lines.map((l) => <li key={l}>{l}</li>)}</ul>
                </div>
              ))}
            </div>
          )}

          <div className="sub-h">Requires <span className="dim mono">{d?.requirementsTotal ?? m.requirements}</span></div>
          {d && d.requirements.length === 0 && d.dlc.length === 0 && <p className="dim" style={{ fontSize: ".9em" }}>Nothing. Add requirements on the page's Requirements step so managers install them too.</p>}
          {d && <div className="rows">
            {d.dlc.map((x) => <div key={x} className="row"><span className="m-main"><span className="m-t">{x}</span><span className="m-meta"><span>DLC</span></span></span></div>)}
            {d.requirements.map((r) => (
              <button key={`${r.modId}-${r.modName}`} type="button" className="row" disabled={r.externalRequirement} onClick={() => openPage(r.modId)}>
                <span className="m-main"><span className="m-t">{r.modName}</span>{r.notes && <span className="m-meta"><span>{r.notes}</span></span>}</span>
                <span className="m-side">{r.externalRequirement ? <span>Off Nexus</span> : <Icon name="web" size={9} />}</span>
              </button>
            ))}
          </div>}

          <div className="sub-h">Required by <span className="dim mono">{formatCount(d?.requiredByTotal ?? m.requiredBy)}</span></div>
          {d && d.requiredBy.length === 0 && <p className="dim" style={{ fontSize: ".9em" }}>No mod lists this one as a requirement yet.</p>}
          {d && <div className="rows">
            {(allBy ? d.requiredBy : d.requiredBy.slice(0, 8)).map((r) => (
              <button key={`${r.modId}-${r.modName}`} type="button" className="row" onClick={() => openPage(r.modId)}>
                <span className="m-main"><span className="m-t">{r.modName}</span>{r.notes && <span className="m-meta"><span>{r.notes}</span></span>}</span>
                <span className="m-side"><Icon name="web" size={9} /></span>
              </button>
            ))}
          </div>}
          {d && d.requiredBy.length > 8 && <Key size="sm" variant="ghost" onClick={() => setAllBy(!allBy)}>{allBy ? "Fewer" : `Show ${d.requiredBy.length - 8} more`}</Key>}
          {d && d.requiredByTotal > d.requiredBy.length && allBy && <p className="dim hint">The first {d.requiredBy.length} of {formatCount(d.requiredByTotal)}.</p>}

          {onUnpin && <div className="actions" style={{ marginTop: "1em" }}><Key size="sm" variant="ghost" icon="x" onClick={onUnpin}>Remove from My mods</Key></div>}
        </Module>
      </div>
    </div>
  );
}
