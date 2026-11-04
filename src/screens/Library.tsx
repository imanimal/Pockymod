// Library: every mod you have, from Pocket Mods, from Vortex, or found in the game, sorted into groups (the app's
// presets or your own). Select any number and turn them on or off, update, move or remove them in one go.
import { useCallback, useEffect, useMemo, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { Empty, Key, Lcd, Module, Pill, SearchField, Seg, Switch, Tag, bytesText } from "../ui/pocket";
import {
  FRAMEWORKS, errorText, groups as groupsApi, library, pickArchive, profiles, queueMod,
  type Conflict, type DeployOutcome, type DownloadRow, type InstallOutcome, type InstallPlan, type InstallRow, type Update,
} from "../lib/api";
import { toast } from "../ui/toast";
import { FomodDialog } from "./FomodDialog";
import type { ChoicesNeeded, TrashRow } from "../lib/api";
import * as ops from "../lib/ops";

type Pending =
  | { kind: "conflicts"; installId: number; name: string; conflicts: Conflict[] }
  | { kind: "copy"; installId: number; name: string; reason: string }
  | { kind: "review"; download: { modId: number; fileId: number; name: string; version: string; archive: string }; plan: InstallPlan };

type Show = "all" | "on" | "off" | "updates";
const SOURCE: Record<InstallRow["source"], string> = { pocket: "", vortex: "Vortex", found: "Found in game" };

export function Library({ updates, onExisting }: { updates?: Update[]; onExisting?: () => void }) {
  const [rows, setRows] = useState<InstallRow[] | null>(null);
  const [downloads, setDownloads] = useState<DownloadRow[]>([]);
  const [customGroups, setCustomGroups] = useState<string[]>([]);
  const [presets, setPresets] = useState<string[]>([]);
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  const [text, setText] = useState("");
  const [show, setShow] = useState<Show>("all");
  const [sel, setSel] = useState<Set<number>>(new Set());
  const [folded, setFolded] = useState<Set<string>>(new Set());
  const [moveOpen, setMoveOpen] = useState(false);
  const [newGroup, setNewGroup] = useState("");
  const [manage, setManage] = useState(false);
  const [choices, setChoices] = useState<ChoicesNeeded | null>(null);
  const [profileName, setProfileName] = useState("");
  const [dupes, setDupes] = useState<number[][]>([]);
  const [trash, setTrash] = useState<TrashRow[]>([]);
  const [showExtra, setShowExtra] = useState(false);
  const [trashOpen, setTrashOpen] = useState(false);
  const [layout, setLayoutState] = useState<"grid" | "list">(() => { try { return localStorage.getItem("pm.libraryLayout") === "list" ? "list" : "grid"; } catch { return "grid"; } });
  const setLayout = (l: "grid" | "list") => { setLayoutState(l); try { localStorage.setItem("pm.libraryLayout", l); } catch { /* storage unavailable */ } };
  const upd = useMemo(() => new Map((updates ?? []).map((u) => [u.modId, u])), [updates]);

  const load = useCallback(() => {
    library.list().then((r) => { setRows(r); setSel((s) => new Set([...s].filter((id) => r.some((x) => x.id === id)))); }).catch((e) => toast(errorText(e), true));
    library.downloads().then(setDownloads).catch(() => {});
    groupsApi.list().then((g) => { setCustomGroups(g.custom); setPresets(g.presets); }).catch(() => {});
    profiles.list().then((p) => setProfileName(p.find((x) => x.active)?.name ?? "")).catch(() => {});
    library.duplicates().then(setDupes).catch(() => setDupes([]));
    library.trash().then(setTrash).catch(() => setTrash([]));
  }, []);
  useEffect(() => {
    load();
    let off: (() => void) | undefined;
    library.onChanged(load).then((u) => { off = u; });
    return () => off?.();
  }, [load]);

  const run = async (f: () => Promise<void>) => { setBusy(true); try { await f(); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); load(); } };

  // extra copies of a mod (two TweakXLs): the one to keep shows, the others wait in the Duplicates notice
  const extra = new Set(dupes.flatMap((d) => d.slice(1)));
  const byId = new Map((rows ?? []).map((r) => [r.id, r]));
  const replacedRows = (rows ?? []).filter((r) => (r.replaced ?? 0) > 0 && r.source === "pocket");
  const visible = (rows ?? []).filter((r) => {
    if (!showExtra && extra.has(r.id)) return false;
    const t = text.trim().toLowerCase();
    if (t && !`${r.name} ${r.category ?? ""} ${r.group} ${r.frameworks.join(" ")}`.toLowerCase().includes(t)) return false;
    if (show === "on" && !r.enabled) return false;
    if (show === "off" && r.enabled) return false;
    if (show === "updates" && !upd.has(r.modId)) return false;
    return true;
  });
  const order = [...customGroups, ...presets];
  const byGroup = new Map<string, InstallRow[]>();
  for (const r of visible) byGroup.set(r.group, [...(byGroup.get(r.group) ?? []), r]);
  const groupNames = [...byGroup.keys()].sort((a, b) => (order.indexOf(a) + 1 || 999) - (order.indexOf(b) + 1 || 999));
  const selected = (rows ?? []).filter((r) => sel.has(r.id));
  const toggle = (id: number) => setSel((s) => { const n = new Set(s); n.has(id) ? n.delete(id) : n.add(id); return n; });
  const toggleAll = (ids: number[]) => setSel((s) => { const all = ids.every((i) => s.has(i)); const n = new Set(s); ids.forEach((i) => all ? n.delete(i) : n.add(i)); return n; });
  const withUpdates = (rows ?? []).filter((r) => upd.has(r.modId) && r.modId < 900_000_000);

  // every change goes through ops, so it can be undone (Ctrl+Z, or the toast's Undo)
  const doBulk = (action: "enable" | "disable" | "uninstall", ids: number[]) => run(async () => {
    const list = (rows ?? []).filter((r) => ids.includes(r.id));
    if (action === "uninstall") { await ops.remove(list); setSel(new Set()); }
    else await ops.setEnabled(list, action === "enable");
  });
  const updateMany = (list: InstallRow[]) => run(async () => {
    let n = 0;
    for (const r of list) { try { await queueMod(r.modId); n++; } catch { /* reported by the queue */ } }
    toast(`${n} ${n === 1 ? "update" : "updates"} queued.`);
  });
  const moveTo = (g: string | null) => run(async () => { await ops.moveToGroup(selected, g); setMoveOpen(false); setNewGroup(""); });
  const useMine = (ids: number[]) => run(async () => {
    const n = await library.restoreReplaced(ids);
    toast(n ? `Your own ${n === 1 ? "copy is" : "copies are"} back in the game.` : "Nothing changed.", n === 0);
  });
  const keepOne = (set: number[]) => run(async () => {
    const keep = byId.get(set[0]);
    await library.keepOne(set[0], set.slice(1));
    toast(`Kept ${keep?.name ?? "one copy"}${keep?.source && keep.source !== "pocket" ? ` (${SOURCE[keep.source]})` : ""}; the extra ${set.length > 2 ? "copies are" : "copy is"} in Recently removed.`);
  });
  const keepAll = () => run(async () => {
    for (const d of dupes) await library.keepOne(d[0], d.slice(1));
    toast(`Cleaned up ${dupes.length} ${dupes.length === 1 ? "mod" : "mods"} with extra copies. They are in Recently removed.`);
  });

  const handleOutcome = (o: InstallOutcome, name: string, download?: Extract<Pending, { kind: "review" }>["download"]) => {
    if (o.status === "installed") {
      toast(`Installed ${name}${o.deploy.outcome === "deployed" && o.deploy.copied ? ` (${o.deploy.copied} files copied)` : ""}.`);
      if (o.needsRedmodDeploy) toast("This is a REDmod: run the REDmod deploy from Load order.");
    } else if (o.status === "needsReview" && download) setPending({ kind: "review", download, plan: o.plan });
    else if (o.status === "staged") onDeployOutcome(o.installId, name, o.deploy);
    else if (o.status === "needsChoices" && download) setChoices({ request: download, fomod: o.fomod, defaults: o.defaults });
    else if (o.status === "alreadyHave") toast(`You already have ${name}. Kept yours.`);
  };
  const onDeployOutcome = (installId: number, name: string, d: DeployOutcome) => {
    if (d.outcome === "conflicts") setPending({ kind: "conflicts", installId, name, conflicts: d.conflicts });
    else if (d.outcome === "needsCopy") setPending({ kind: "copy", installId, name, reason: d.reason });
    else { toast(`Deployed ${name}.`); setPending(null); }
  };
  const deploy = (r: { id: number; name: string }, takeConflicts = false, allowCopy = false) =>
    run(async () => onDeployOutcome(r.id, r.name, await library.deploy(r.id, { takeConflicts, allowCopy })));
  const installDownload = (d: { modId: number; fileId: number; name: string; version: string; archive: string }, acceptUnknown = false) =>
    run(async () => handleOutcome(await library.installFile({ ...d, acceptUnknown }), d.name, d));
  const installFromFile = () => run(async () => {
    const archive = await pickArchive();
    if (!archive) return;
    const name = archive.split(/[\\/]/).pop()?.replace(/\.(zip|7z|rar)$/i, "") ?? "Mod";
    const local = 900_000_000 + (Array.from(name).reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7) % 90_000_000);
    const d = { modId: local, fileId: local, name, version: "local", archive };
    handleOutcome(await library.installFile(d), name, d);
  });

  const notInstalled = downloads.filter((d) => !d.installed);
  const on = (rows ?? []).filter((r) => r.enabled).length;

  return (
    <div className="stack lib">
      <div className="lcds">
        <Lcd size="sm" label="Mods" value={String(rows?.length ?? 0).padStart(2, "0")} sub={profileName ? `profile ${profileName}` : undefined} />
        <Lcd size="sm" label="Turned on" value={String(on).padStart(2, "0")} />
        <Lcd size="sm" label="Updates" value={String(withUpdates.length).padStart(2, "0")} tone={withUpdates.length ? "hot" : "ok"} />
      </div>

      <div className="browse-top">
        <SearchField value={text} onChange={setText} placeholder="Find in your mods" />
        <Seg label="Layout" value={layout} onChange={setLayout} options={[{ value: "grid", label: "Grid" }, { value: "list", label: "List" }]} />
        <Seg label="Show" value={show} onChange={setShow} options={[{ value: "all", label: "All" }, { value: "on", label: "On" }, { value: "off", label: "Off" }, { value: "updates", label: `Updates${withUpdates.length ? ` ${withUpdates.length}` : ""}` }]} />
        {withUpdates.length > 0 && <Key icon="downloads" variant="primary" disabled={busy} onClick={() => updateMany(withUpdates)}>Update all ({withUpdates.length})</Key>}
        <Key size="sm" icon="file" onClick={installFromFile} disabled={busy}>Install from file</Key>
        <Key size="sm" icon="folder" onClick={() => onExisting?.()} title="Find mods Vortex installed or that are in the game folder already">Find existing mods</Key>
        <Key size="sm" variant="ghost" icon="settings" latched={manage} onClick={() => setManage(!manage)}>Groups</Key>
      </div>

      {sel.size > 0 && (
        <div className="bulkbar" role="toolbar" aria-label="Selected mods">
          <span className="display sm inline"><span className="d-lbl">Selected</span><span className="d-num">{String(sel.size).padStart(2, "0")}</span></span>
          <Key size="sm" icon="check" disabled={busy} onClick={() => doBulk("enable", [...sel])}>Turn on</Key>
          <Key size="sm" icon="minus" disabled={busy} onClick={() => doBulk("disable", [...sel])}>Turn off</Key>
          {selected.some((r) => upd.has(r.modId)) && <Key size="sm" icon="downloads" disabled={busy} onClick={() => updateMany(selected.filter((r) => upd.has(r.modId)))}>Update</Key>}
          <Key size="sm" icon="folder" disabled={busy} onClick={() => setMoveOpen(true)}>Move to group</Key>
          <Key size="sm" variant="ghost" icon="trash" disabled={busy} onClick={() => doBulk("uninstall", [...sel])}>Remove</Key>
          <span className="sp" />
          <Key size="sm" variant="ghost" onClick={() => setSel(new Set())}>Clear selection</Key>
        </div>
      )}

      {manage && <GroupManager custom={customGroups} onChange={load} />}

      {replacedRows.length > 0 && (
        <Module icon="warn" title="Your own files were set aside" count={replacedRows.length}>
          <p>When these went in, files you already had in the game (yours, or Vortex's) were set aside to make room. You can go back to yours: these copies come out and yours are tracked as your own mods.</p>
          <div className="rows" style={{ marginTop: ".5em" }}>
            {replacedRows.map((r) => (
              <div key={r.id} className="row" style={{ cursor: "default" }}>
                <span className="m-main"><span className="m-t">{r.name}</span>
                  <span className="m-meta"><span>{r.replaced} {r.replaced === 1 ? "file" : "files"} of yours set aside</span>{r.origin && <span>installed for {r.origin}</span>}</span></span>
                <span className="m-side" style={{ flexDirection: "row", gap: ".3em" }}><Key size="sm" disabled={busy} onClick={() => useMine([r.id])}>Use mine</Key></span>
              </div>
            ))}
          </div>
          <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".5em" }}>
            {replacedRows.length > 1 && <Key icon="check" disabled={busy} onClick={() => useMine(replacedRows.map((r) => r.id))}>Use mine for all {replacedRows.length}</Key>}
          </div>
        </Module>
      )}

      {dupes.length > 0 && (
        <Module icon="library" title="Mods you have twice" count={dupes.length}
          tools={<Key size="sm" variant="ghost" icon={showExtra ? "hide" : "eye"} onClick={() => setShowExtra(!showExtra)}>{showExtra ? "Hide extra copies" : "Show extra copies"}</Key>}>
          <p className="dim" style={{ fontSize: ".9em" }}>Only one copy of each shows below: the one that is on, your own before one added for another mod. Keep one to tidy up; the others go to Recently removed (a Vortex copy stays in Vortex).</p>
          <div className="rows" style={{ marginTop: ".5em" }}>
            {dupes.map((d) => {
              const keep = byId.get(d[0]);
              const others = d.slice(1).map((id) => byId.get(id)).filter((x): x is InstallRow => !!x);
              if (!keep) return null;
              const tag = (r: InstallRow) => `${r.version ? `v${r.version}` : "version unknown"}, ${r.source === "pocket" ? (r.origin ? `added for ${r.origin}` : "Pocket Mods") : SOURCE[r.source]}${r.enabled ? ", on" : ", off"}`;
              return (
                <div key={d[0]} className="row" style={{ cursor: "default" }}>
                  <span className="m-main"><span className="m-t">{keep.name}</span>
                    <span className="m-meta"><span className="okc">keep: {tag(keep)}</span>{others.map((o) => <span key={o.id}>extra: {tag(o)}</span>)}</span></span>
                  <span className="m-side" style={{ flexDirection: "row", gap: ".3em" }}><Key size="sm" disabled={busy} onClick={() => keepOne(d)}>Keep this one</Key></span>
                </div>
              );
            })}
          </div>
          {dupes.length > 1 && <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".5em" }}><Key icon="check" disabled={busy} onClick={keepAll}>Tidy up all {dupes.length}</Key></div>}
        </Module>
      )}

      {rows?.length === 0 && (
        <Module icon="library" title="Your mods">
          <Empty icon="library" title="No mods yet" actions={<Key icon="folder" onClick={() => onExisting?.()}>Find mods I already have</Key>}>
            Install from Browse, or bring in the mods Vortex or you already put in the game.
          </Empty>
        </Module>
      )}

      <div className="lib-groups">
      {groupNames.map((g) => {
        const list = byGroup.get(g)!;
        const ids = list.map((r) => r.id);
        const allSel = ids.every((i) => sel.has(i));
        const isFolded = folded.has(g);
        // small groups sit side by side on wide windows instead of leaving a row mostly empty
        const basis = layout === "grid" ? `${Math.max(2, Math.min(list.length, 6)) * 13.3 + 1.4}em` : "46em";
        return (
          <div key={g} className="lib-group" style={{ flexBasis: basis }}>
          <Module icon={g === "Core" ? "chip" : g === "Character presets" ? "user" : "library"} title={g}
            count={`${list.filter((r) => r.enabled).length} of ${list.length} on`} bodyClass="list"
            tools={<>
              <Key size="sm" variant="ghost" onClick={() => toggleAll(ids)}>{allSel ? "Unselect" : "Select all"}</Key>
              <Key size="sm" variant="ghost" icon="chevron" className={isFolded ? "" : "rot-key"} aria-label={isFolded ? "Open" : "Fold"}
                onClick={() => setFolded((f) => { const n = new Set(f); n.has(g) ? n.delete(g) : n.add(g); return n; })} />
            </>}>
            {!isFolded && layout === "grid" && (
              <div className="lgrid2">
                {list.map((r) => (
                  <div key={r.id} className={`lcard${r.enabled ? "" : " off"}${sel.has(r.id) ? " sel" : ""}`} role="button" tabIndex={0} aria-pressed={sel.has(r.id)}
                    onClick={() => toggle(r.id)} onKeyDown={(e) => { if (e.key === " " || e.key === "Enter") { e.preventDefault(); toggle(r.id); } }}>
                    <span className="lcard-img">
                      {r.picture ? <img src={r.picture.replace("/images/", "/images/thumbnails/")} alt="" loading="lazy" onError={(e) => { const i = e.target as HTMLImageElement; if (r.picture && i.src !== r.picture) i.src = r.picture; }} /> : <Icon name={r.modType === "framework" ? "chip" : "file"} size={27} />}
                      <input type="checkbox" className="pick" checked={sel.has(r.id)} onChange={() => toggle(r.id)} onClick={(e) => e.stopPropagation()} aria-label={`Select ${r.name}`} />
                      {upd.get(r.modId) && <span className="lcard-badge"><Pill tone="soon">Update</Pill></span>}
                    </span>
                    <span className="lcard-b">
                      <span className="m-t">{r.name}</span>
                      <span className="m-meta">{r.version && <span>v{r.version}</span>}{SOURCE[r.source] && <span>{SOURCE[r.source]}</span>}</span>
                      <span className="lcard-f" onClick={(e) => e.stopPropagation()}>
                        <Switch checked={r.enabled} onChange={(v) => doBulk(v ? "enable" : "disable", [r.id])} disabled={busy}>{r.enabled ? "On" : "Off"}</Switch>
                        {upd.get(r.modId) && r.modId < 900_000_000 && <Key size="sm" icon="downloads" onClick={() => run(async () => { await queueMod(r.modId); toast(`${r.name} update queued.`); })} disabled={busy} aria-label={`Update ${r.name}`} />}
                      </span>
                    </span>
                  </div>
                ))}
              </div>
            )}
            {!isFolded && layout === "list" && (
              <div className="rows">
                {list.map((r) => (
                  <div key={r.id} className={`row librow${r.enabled ? "" : " off"}${sel.has(r.id) ? " sel" : ""}`}>
                    <input type="checkbox" className="pick" checked={sel.has(r.id)} onChange={() => toggle(r.id)} aria-label={`Select ${r.name}`} />
                    <span className="m-main">
                      <span className="m-t">{r.name}</span>
                      <span className="m-meta">
                        {r.version && <span>v{r.version}</span>}
                        <span>{r.deployedFiles} files in the game</span>
                        {r.category && <span>{r.category}</span>}
                        {SOURCE[r.source] && <span><Pill tone={r.source === "vortex" ? "soon" : undefined}>{SOURCE[r.source]}</Pill></span>}
                        {upd.get(r.modId) && <span><Pill tone="soon">Update {upd.get(r.modId)!.latest}</Pill></span>}
                      </span>
                      <span className="m-tags">
                        <Tag kind={r.modType === "unknown" || r.modType === "mixed" ? undefined : r.modType} square>{r.modType === "redmod" ? "REDmod" : r.modType}</Tag>
                        {r.frameworks.map((f) => <Tag key={f} kind={f}>{FRAMEWORKS.find((x) => x.key === f)?.short ?? f}</Tag>)}
                      </span>
                    </span>
                    <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".4em" }}>
                      {upd.get(r.modId) && r.modId < 900_000_000 && <Key size="sm" icon="downloads" onClick={() => run(async () => { await queueMod(r.modId); toast(`${r.name} update queued.`); })} disabled={busy}>Update</Key>}
                      {r.enabled && r.deployedFiles === 0 && <Key size="sm" onClick={() => deploy(r)} disabled={busy}>Deploy</Key>}
                      <Switch checked={r.enabled} onChange={(v) => doBulk(v ? "enable" : "disable", [r.id])} disabled={busy}><span className="sr-only">On</span></Switch>
                      <Key size="sm" variant="ghost" icon="trash" onClick={() => doBulk("uninstall", [r.id])} disabled={busy} aria-label={`Remove ${r.name}`} title={r.source === "vortex" ? "Remove from the game; it stays in Vortex" : "Remove"} />
                    </span>
                  </div>
                ))}
              </div>
            )}
          </Module>
          </div>
        );
      })}
      </div>

      {notInstalled.length > 0 && (
        <Module icon="downloads" title="Downloaded, not installed" count={notInstalled.length} bodyClass="list">
          <div className="rows">
            {notInstalled.map((d) => (
              <div key={`${d.modId}-${d.fileId}`} className="row">
                <span className="m-main"><span className="m-t">{d.name}</span><span className="m-meta"><span>v{d.version}</span><span className="mono">{d.md5.slice(0, 12)}</span></span></span>
                <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".5em" }}>
                  <span>{bytesText(d.size)}</span>
                  <Key size="sm" icon="install" disabled={busy} onClick={() => installDownload({ modId: d.modId, fileId: d.fileId, name: d.name, version: d.version, archive: d.path })}>Install</Key>
                </span>
              </div>
            ))}
          </div>
        </Module>
      )}

      {trash.length > 0 && (
        <Module id="recently-removed" icon="trash" title="Recently removed" count={trash.length} bodyClass="list"
          tools={<>
            <Key size="sm" variant="ghost" icon="chevron" className={trashOpen ? "rot-key" : ""} aria-label={trashOpen ? "Fold" : "Open"} onClick={() => setTrashOpen(!trashOpen)} />
          </>}>
          {!trashOpen ? (
            <button type="button" className="linkish fold-line" onClick={() => setTrashOpen(true)}>
              {trash.slice(0, 3).map((t) => t.name).join(", ")}{trash.length > 3 ? ` and ${trash.length - 3} more` : ""}: kept {14} days, then deleted. Open to put any back.
            </button>
          ) : (
            <>
              <div className="rows">
                {trash.map((t) => (
                  <div key={t.id} className="row" style={{ cursor: "default" }}>
                    <span className="m-main"><span className="m-t">{t.name}</span>
                      <span className="m-meta">{t.version && <span>v{t.version}</span>}<span>{t.reason === "updated" ? "older version, replaced by an update" : t.reason}</span><span>{ago(t.removedAt)}</span>{t.size > 0 && <span>{bytesText(t.size)}</span>}{t.source === "vortex" && <span>still in Vortex</span>}</span></span>
                    <span className="m-side" style={{ flexDirection: "row", gap: ".3em" }}>
                      <Key size="sm" icon="undo" disabled={busy} onClick={() => run(async () => { await ops.restoreRemoved([t.id], [t.name]); })}>Put back</Key>
                      <Key size="sm" variant="ghost" icon="x" disabled={busy} aria-label={`Delete ${t.name} for good`} title="Delete for good" onClick={() => run(async () => { await library.purge(t.id); })} />
                    </span>
                  </div>
                ))}
              </div>
              <div className="actions" style={{ justifyContent: "flex-end", padding: ".5em .8em" }}>
                <Key size="sm" variant="ghost" icon="trash" disabled={busy} onClick={() => run(async () => { const n = await library.purge(undefined, true); toast(`Deleted ${n} removed ${n === 1 ? "mod" : "mods"} for good.`); })}>Empty</Key>
              </div>
            </>
          )}
        </Module>
      )}

      {choices && <FomodDialog need={choices} onClose={() => setChoices(null)} onInstalled={load} />}
      {moveOpen && (
        <Dialog icon="folder" title="Move to group" count={`${sel.size} mods`} onClose={() => setMoveOpen(false)}>
          <div className="chips">
            {customGroups.map((g) => <button key={g} type="button" className="chip" onClick={() => moveTo(g)}>{g}</button>)}
            <button type="button" className="chip" onClick={() => moveTo(null)}>Back to their own group</button>
          </div>
          <div className="actions" style={{ flexWrap: "nowrap" }}>
            <label className="fbox"><input className="finput" value={newGroup} onChange={(e) => setNewGroup(e.target.value)} placeholder="A new group, e.g. Night look" aria-label="New group" onKeyDown={(e) => { if (e.key === "Enter" && newGroup.trim()) moveTo(newGroup); }} /></label>
            <Key icon="plus" disabled={!newGroup.trim() || busy} onClick={() => moveTo(newGroup)}>Make and move</Key>
          </div>
          <p className="dim" style={{ fontSize: ".84em" }}>The app's groups ({presets.join(", ")}) fill themselves from each mod's category. Your own groups come first.</p>
        </Dialog>
      )}
      {pending?.kind === "conflicts" && (
        <Dialog icon="warn" title="Files already in use" count={`${pending.conflicts.length} files`} band="bad" onClose={() => setPending(null)}>
          <p>{pending.name} wants files that other mods have put in the game. Nothing has been changed yet.</p>
          <div className="log" style={{ maxHeight: "10em" }}>{pending.conflicts.slice(0, 50).map((c) => `${c.target}  ·  ${c.ownerName}`).join("\n")}</div>
          <p className="dim" style={{ fontSize: ".86em" }}>Taking them over keeps the other mods' copies. They get their files back if you remove {pending.name} later.</p>
          <div className="actions" style={{ justifyContent: "flex-end" }}>
            <Key variant="ghost" onClick={() => setPending(null)}>Leave it staged</Key>
            <Key onClick={() => deploy({ id: pending.installId, name: pending.name }, true)} disabled={busy}>Take them over</Key>
          </div>
        </Dialog>
      )}
      {pending?.kind === "copy" && (
        <Dialog icon="warn" title="Hard links are not possible" onClose={() => setPending(null)}>
          <p>{pending.reason}</p>
          <div className="note warn">Copying uses disk space twice and, on the C drive, fills the system disk. Moving the mod library to the game's drive in Settings is better.</div>
          <div className="actions" style={{ justifyContent: "flex-end" }}>
            <Key variant="ghost" onClick={() => setPending(null)}>Cancel</Key>
            <Key onClick={() => deploy({ id: pending.installId, name: pending.name }, false, true)} disabled={busy}>Copy this once</Key>
          </div>
        </Dialog>
      )}
      {pending?.kind === "review" && (
        <Dialog icon="warn" title="Some files have no known place" count={`${pending.plan.unknown.length} files`} onClose={() => setPending(null)}>
          <p>{pending.download.name} has files the installer does not know where to put. They will be left out; the rest goes in as planned.</p>
          <div className="log" style={{ maxHeight: "10em" }}>{pending.plan.unknown.slice(0, 60).join("\n")}</div>
          <div className="actions" style={{ justifyContent: "flex-end" }}>
            <Key variant="ghost" onClick={() => setPending(null)}>Cancel</Key>
            <Key onClick={() => { const d = pending.download; setPending(null); installDownload(d, true); }} disabled={busy}>Install the rest</Key>
          </div>
        </Dialog>
      )}
    </div>
  );
}

function ago(unix: number): string {
  const s = Math.max(0, Date.now() / 1000 - unix);
  if (s < 90) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  if (s < 86400) return `${Math.round(s / 3600)} h ago`;
  return `${Math.round(s / 86400)} days ago`;
}

function GroupManager({ custom, onChange }: { custom: string[]; onChange: () => void }) {
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [to, setTo] = useState("");
  const act = (p: Promise<unknown>) => p.then(onChange).catch((e) => toast(errorText(e), true));
  return (
    <Module icon="folder" title="Your groups" count={custom.length || undefined}>
      <p className="dim" style={{ fontSize: ".86em", marginBottom: ".5em" }}>Make groups of your own (a look, a playthrough, mods to test). Select mods and use Move to group. Deleting a group sends its mods back to the app's groups.</p>
      <div className="rows">
        {custom.map((g) => (
          <div key={g} className="row" style={{ cursor: "default" }}>
            {editing === g ? (
              <label className="fbox"><input className="finput" autoFocus value={to} onChange={(e) => setTo(e.target.value)} aria-label="New name"
                onKeyDown={(e) => { if (e.key === "Enter" && to.trim()) { act(groupsApi.rename(g, to)); setEditing(null); } if (e.key === "Escape") setEditing(null); }} /></label>
            ) : <span className="m-main"><span className="m-t"><Icon name="folder" size={9} /> {g}</span></span>}
            <span className="m-side" style={{ flexDirection: "row", gap: ".3em" }}>
              <Key size="sm" variant="ghost" onClick={() => { setEditing(g); setTo(g); }}>Rename</Key>
              <Key size="sm" variant="ghost" icon="trash" aria-label={`Delete ${g}`} onClick={() => act(groupsApi.remove(g))} />
            </span>
          </div>
        ))}
      </div>
      <div className="actions" style={{ marginTop: ".5em", flexWrap: "nowrap" }}>
        <label className="fbox"><input className="finput" value={name} onChange={(e) => setName(e.target.value)} placeholder="New group" aria-label="New group name" /></label>
        <Key size="sm" icon="plus" disabled={!name.trim()} onClick={() => { act(groupsApi.create(name)); setName(""); }}>Add</Key>
      </div>
    </Module>
  );
}
