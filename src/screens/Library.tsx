// Library: every mod you have, from Pockymod, from Vortex, or found in the game, sorted into groups (the app's
// presets or your own). Select any number and turn them on or off, update, move or remove them in one go.
import { plural } from "../lib/text";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { Empty, Key, Lcd, Module, Pill, SearchField, Seg, Switch, Tag, bytesText } from "../ui/pocket";
import {
  FRAMEWORKS, errorText, groups as groupsApi, library, pickArchive, profiles, queue, queueMod, unmanagedList, updates as updatesApi, vortexProfiles, type Unmanaged,
  type Conflict, type DeployOutcome, type DownloadRow, type InstallOutcome, type InstallPlan, type InstallRow, type Update,
} from "../lib/api";
import { toast } from "../ui/toast";
import { FomodDialog } from "./FomodDialog";
import type { ChoicesNeeded, TrashRow } from "../lib/api";
import { planGroups, useExtensions } from "../lib/extensions";
import * as ops from "../lib/ops";
import { installArchive } from "../lib/archives";
import { installerOptions } from "../lib/api";
import { record } from "../lib/history";
import { profilesText } from "./Existing";
import { EndorseStrip, endorseAll, useCandidates } from "../ui/Endorse";
import { SettingsLink } from "../ui/SettingsLink";

type Pending =
  | { kind: "conflicts"; installId: number; name: string; conflicts: Conflict[] }
  | { kind: "copy"; installId: number; name: string; reason: string }
  | { kind: "review"; download: { modId: number; fileId: number; name: string; version: string; archive: string }; plan: InstallPlan };

type Show = "all" | "on" | "off" | "updates";
const SOURCE: Record<InstallRow["source"], string> = { pocket: "", vortex: "Vortex", found: "Unmanaged" };
type From = "all" | "vortex" | "pocket" | "found";
/** Unmanaged mods set aside with Later: listed last in a folded group of their own. */
const LATER = "Unmanaged, for later";

export function Library({ updates, onExisting }: { updates?: Update[]; onExisting?: () => void }) {
  const [rows, setRows] = useState<InstallRow[] | null>(null);
  const [downloads, setDownloads] = useState<DownloadRow[]>([]);
  const [customGroups, setCustomGroups] = useState<string[]>([]);
  const [presets, setPresets] = useState<string[]>([]);
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  const [text, setText] = useState("");
  const [show, setShow] = useState<Show>("all");
  const [from, setFrom] = useState<From>("all");
  const [unm, setUnm] = useState<Unmanaged | null>(null);
  const [unmLater, setUnmLaterState] = useState(() => { try { return localStorage.getItem("pm.unmanaged.later") === "1"; } catch { return false; } });
  const setUnmLater = (v: boolean) => { setUnmLaterState(v); try { localStorage.setItem("pm.unmanaged.later", v ? "1" : "0"); } catch { /* storage unavailable */ } };
  const [sel, setSel] = useState<Set<number>>(new Set());
  const [folded, setFolded] = useState<Set<string>>(new Set());
  const [moveOpen, setMoveOpen] = useState(false);
  const [newGroup, setNewGroup] = useState("");
  const [manage, setManage] = useState(false);
  const [choices, setChoices] = useState<ChoicesNeeded | null>(null);
  const [reopt, setReopt] = useState<{ row: InstallRow; need: ChoicesNeeded; before: ChoicesNeeded["defaults"] } | null>(null);
  const [profileName, setProfileName] = useState("");
  const [dupes, setDupes] = useState<number[][]>([]);
  const [trash, setTrash] = useState<TrashRow[]>([]);
  const [showExtra, setShowExtra] = useState(false);
  const [trashOpen, setTrashOpen] = useState(false);
  // Vortex's profiles, when its mods were brought in before profiles came along with them
  const [vxp, setVxp] = useState<{ names: string[]; tracked: number } | null>(null);
  const groupRules = useExtensions().active.groupRules;
  // mods not endorsed yet get a one-click star
  const toEndorse = new Set(useCandidates().list.map((c) => c.modId));
  const star = (r: InstallRow) => toEndorse.has(r.modId) && (
    <Key size="sm" variant="ghost" icon="star" disabled={busy} aria-label={`Endorse ${r.name}`} title={`Endorse ${r.name} on Nexus`}
      onClick={() => void endorseAll([{ modId: r.modId, name: r.name, version: r.version }])} />
  );
  const [sortPlan, setSortPlan] = useState<ReturnType<typeof planGroups> | null>(null);
  const [layout, setLayoutState] = useState<"grid" | "list">(() => { try { return localStorage.getItem("pm.libraryLayout") === "list" ? "list" : "grid"; } catch { return "grid"; } });
  const setLayout = (l: "grid" | "list") => { setLayoutState(l); try { localStorage.setItem("pm.libraryLayout", l); } catch { /* storage unavailable */ } };
  // updates are per install (the main file and an optional one of the same mod can each have one)
  const upd = useMemo(() => {
    const byInstall = new Map<number, Update>(); const byMod = new Map<number, Update>();
    for (const u of updates ?? []) { if (u.installId) byInstall.set(u.installId, u); else byMod.set(u.modId, u); }
    return { byInstall, byMod };
  }, [updates]);
  const updOf = (r: InstallRow): Update | undefined => upd.byInstall.get(r.id) ?? upd.byMod.get(r.modId);

  const load = useCallback(() => {
    library.list().then((r) => { setRows(r); setSel((s) => new Set([...s].filter((id) => r.some((x) => x.id === id)))); }).catch((e) => toast(errorText(e), true));
    library.downloads().then(setDownloads).catch(() => {});
    groupsApi.list().then((g) => { setCustomGroups(g.custom); setPresets(g.presets); }).catch(() => {});
    profiles.list().then((p) => setProfileName(p.find((x) => x.active)?.name ?? "")).catch(() => {});
    library.duplicates().then(setDupes).catch(() => setDupes([]));
    library.trash().then(setTrash).catch(() => setTrash([]));
    unmanagedList().then(setUnm).catch(() => setUnm(null));
    vortexProfiles.scan().then((v) => {
      let later = false;
      try { later = localStorage.getItem("pm.vortexProfiles.later") === "1"; } catch { /* storage unavailable */ }
      setVxp(!v.imported && !later && v.tracked > 0 && v.profiles.length > 1 ? { names: v.profiles.map((p) => p.name), tracked: v.tracked } : null);
    }).catch(() => setVxp(null));
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
    if (show === "updates" && !!!updOf(r)) return false;
    if (from !== "all" && r.source !== from) return false;
    return true;
  });
  const order = [...customGroups, ...presets];
  // set aside with Later: the leftover copies and other unmanaged mods wait at the bottom (frameworks stay put)
  const laterIds = new Set(unmLater && unm ? [...unm.copies, ...unm.other].map((c) => c[0]) : []);
  const byGroup = new Map<string, InstallRow[]>();
  for (const r of visible) { const g = laterIds.has(r.id) ? LATER : r.group; byGroup.set(g, [...(byGroup.get(g) ?? []), r]); }
  const rank = (g: string) => (g === LATER ? 1000 : order.indexOf(g) + 1 || 999);
  const groupNames = [...byGroup.keys()].sort((a, b) => rank(a) - rank(b));
  const selected = (rows ?? []).filter((r) => sel.has(r.id));
  const toggle = (id: number) => setSel((s) => { const n = new Set(s); n.has(id) ? n.delete(id) : n.add(id); return n; });
  const toggleAll = (ids: number[]) => setSel((s) => { const all = ids.every((i) => s.has(i)); const n = new Set(s); ids.forEach((i) => all ? n.delete(i) : n.add(i)); return n; });
  const withUpdates = (rows ?? []).filter((r) => !!updOf(r) && r.modId < 900_000_000);

  // every change goes through ops, so it can be undone (Ctrl+Z, or the toast's Undo)
  const doBulk = (action: "enable" | "disable" | "uninstall", ids: number[]) => run(async () => {
    const list = (rows ?? []).filter((r) => ids.includes(r.id));
    if (action === "uninstall") { await ops.remove(list); setSel(new Set()); }
    else await ops.setEnabled(list, action === "enable");
  });
  // an update queues the file that follows the installed one (2K stays 2K)
  const queueUpdate = (r: InstallRow) => { const u = updOf(r); return u?.fileId ? queue.add(u.modId, u.fileId, true) : queueMod(r.modId); };
  const updateMany = (list: InstallRow[]) => run(async () => {
    let n = 0;
    for (const r of list) { try { await queueUpdate(r); n++; } catch { /* reported by the queue */ } }
    toast(`${n} ${n === 1 ? "update" : "updates"} queued`);
  });
  const moveTo = (g: string | null) => run(async () => { await ops.moveToGroup(selected, g); setMoveOpen(false); setNewGroup(""); });
  const useMine = (ids: number[]) => run(async () => {
    const n = await library.restoreReplaced(ids);
    toast(n ? `Your own ${n === 1 ? "copy is" : "copies are"} back in the game` : "Nothing changed", n === 0);
  });
  const keepOne = (set: number[]) => run(async () => {
    const keep = byId.get(set[0]);
    await library.keepOne(set[0], set.slice(1));
    toast(`Kept ${keep?.name ?? "one copy"}${keep?.source && keep.source !== "pocket" ? ` (${SOURCE[keep.source]})` : ""}. Extra ${set.length > 2 ? "copies" : "copy"} in Recently removed`);
  });
  const keepAll = () => run(async () => {
    for (const d of dupes) await library.keepOne(d[0], d.slice(1));
    toast(`Cleaned up ${dupes.length} ${dupes.length === 1 ? "mod" : "mods"}. Extras in Recently removed`);
  });

  const handleOutcome = (o: InstallOutcome, name: string, download?: Extract<Pending, { kind: "review" }>["download"]) => {
    if (o.status === "installed") {
      toast(`Installed ${name}${o.deploy.outcome === "deployed" && o.deploy.copied ? ` (${plural(o.deploy.copied, "file")} copied)` : ""}`);
      if (o.needsRedmodDeploy) toast("REDmod. Deploy it from Load order");
    } else if (o.status === "needsReview" && download) setPending({ kind: "review", download, plan: o.plan });
    else if (o.status === "staged") onDeployOutcome(o.installId, name, o.deploy);
    else if (o.status === "needsChoices" && download) setChoices({ request: download, fomod: o.fomod, defaults: o.defaults });
    else if (o.status === "alreadyHave") toast(`Already have ${name}`);
  };
  const onDeployOutcome = (installId: number, name: string, d: DeployOutcome) => {
    if (d.outcome === "conflicts") setPending({ kind: "conflicts", installId, name, conflicts: d.conflicts });
    else if (d.outcome === "needsCopy") setPending({ kind: "copy", installId, name, reason: d.reason });
    else { toast(`Deployed ${name}`); setPending(null); }
  };
  const deploy = (r: { id: number; name: string }, takeConflicts = false, allowCopy = false) =>
    run(async () => onDeployOutcome(r.id, r.name, await library.deploy(r.id, { takeConflicts, allowCopy })));
  const installDownload = (d: { modId: number; fileId: number; name: string; version: string; archive: string }, acceptUnknown = false) =>
    run(async () => handleOutcome(await library.installFile({ ...d, acceptUnknown }), d.name, d));
  const installFromFile = () => run(async () => {
    const archive = await pickArchive();
    if (!archive) return;
    const { outcome, request } = await installArchive(archive);
    handleOutcome(outcome, request.name, request);
  });

  // a FOMOD mod's installer again, its last picks selected; the new picks go in from the staged files
  const changeOptions = (r: InstallRow) => run(async () => {
    const o = await installerOptions.get(r.id);
    if (!o.saved) toast(`No saved picks for ${r.name}, showing the installer defaults`);
    const request = { modId: r.modId, fileId: r.fileId, name: r.name, version: r.version, archive: "" };
    setReopt({ row: r, need: { request, fomod: o.fomod, defaults: o.choices }, before: o.choices });
  });
  const applyOptions = async (r: InstallRow, picks: ChoicesNeeded["defaults"], before: ChoicesNeeded["defaults"]) => {
    const d = await installerOptions.change(r.id, picks);
    if (d && d.outcome !== "deployed") onDeployOutcome(r.id, r.name, d);
    record({ label: `Changed the options of ${r.name}`, undo: () => installerOptions.change(r.id, before), redo: () => installerOptions.change(r.id, picks) }, !!d && d.outcome !== "deployed");
    if (!d) toast(`Options saved. They apply when ${r.name} is turned on`);
    load();
  };

  const notInstalled = downloads.filter((d) => !d.installed);
  const on = (rows ?? []).filter((r) => r.enabled).length;

  return (
    <div className="stack lib">
      <div className="lcds">
        <Lcd size="sm" label="Mods" value={String(rows?.length ?? 0).padStart(2, "0")}
          sub={[profileName && `profile ${profileName}`, (rows ?? []).some((r) => r.source === "vortex") && `${(rows ?? []).filter((r) => r.source === "vortex").length} Vortex`,
            (rows ?? []).some((r) => r.source === "found") && `${(rows ?? []).filter((r) => r.source === "found").length} unmanaged`].filter(Boolean).join(" · ") || undefined} />
        <Lcd size="sm" label="Turned on" value={String(on).padStart(2, "0")} />
        <Lcd size="sm" label="Updates" value={String(withUpdates.length).padStart(2, "0")} tone={withUpdates.length ? "hot" : "ok"} />
      </div>

      <div className="browse-top">
        <SearchField value={text} onChange={setText} placeholder="Find in your mods" />
        <Seg label="Layout" value={layout} onChange={setLayout} options={[{ value: "grid", label: "Grid" }, { value: "list", label: "List" }]} />
        <Seg label="From" value={from} onChange={setFrom} options={[{ value: "all", label: "All" }, { value: "vortex", label: "Vortex", title: "Mods Vortex installed" },
          { value: "pocket", label: "Pockymod", title: "Mods installed here" }, { value: "found", label: "Unmanaged", title: "In the game folder, installed by neither manager" }]} />
        <Seg label="Show" value={show} onChange={setShow} options={[{ value: "all", label: "All" }, { value: "on", label: "On" }, { value: "off", label: "Off" }, { value: "updates", label: `Updates${withUpdates.length ? ` ${withUpdates.length}` : ""}` }]} />
        {withUpdates.length > 0 && <Key icon="downloads" variant="primary" disabled={busy} onClick={() => updateMany(withUpdates)}>Update all ({withUpdates.length})</Key>}
        <Key size="sm" icon="file" onClick={installFromFile} disabled={busy}>Install from file</Key>
        <Key size="sm" icon="folder" onClick={() => onExisting?.()} title="Vortex mods and mods already in the game folder">Find existing mods</Key>
        <Key size="sm" variant="ghost" icon="settings" latched={manage} onClick={() => setManage(!manage)}>Groups</Key>
        {groupRules.length > 0 && <Key size="sm" icon="folder" disabled={busy || !rows?.length} onClick={() => setSortPlan(planGroups(rows ?? [], groupRules))} title="Apply your extensions' group rules">Sort into groups</Key>}
      </div>

      {sel.size > 0 && (
        <div className="bulkbar" role="toolbar" aria-label="Selected mods">
          <span className="display sm inline"><span className="d-lbl">Selected</span><span className="d-num">{String(sel.size).padStart(2, "0")}</span></span>
          <Key size="sm" icon="check" disabled={busy} onClick={() => doBulk("enable", [...sel])}>Turn on</Key>
          <Key size="sm" icon="minus" disabled={busy} onClick={() => doBulk("disable", [...sel])}>Turn off</Key>
          {selected.some((r) => !!updOf(r)) && <Key size="sm" icon="downloads" disabled={busy} onClick={() => updateMany(selected.filter((r) => !!updOf(r)))}>Update</Key>}
          <Key size="sm" icon="folder" disabled={busy} onClick={() => setMoveOpen(true)}>Move to group</Key>
          {selected.some((r) => r.modId < 900_000_000) && <Key size="sm" icon="star" disabled={busy} title="Endorse the selected mods on Nexus"
            onClick={() => run(() => endorseAll(selected.filter((r) => r.modId < 900_000_000).filter((r, n, a) => a.findIndex((x) => x.modId === r.modId) === n).map((r) => ({ modId: r.modId, name: r.name, version: r.version }))))}>Endorse</Key>}
          <Key size="sm" variant="ghost" icon="trash" disabled={busy} onClick={() => doBulk("uninstall", [...sel])}>Remove</Key>
          <span className="sp" />
          <Key size="sm" variant="ghost" onClick={() => setSel(new Set())}>Clear selection</Key>
        </div>
      )}

      {manage && <GroupManager custom={customGroups} onChange={load} />}
      <EndorseStrip />
      {unm && !unmLater && unm.copies.length + unm.other.length + unm.frameworks.length > 0 && (() => {
        const rowsOf = (ids: number[]) => (rows ?? []).filter((r) => ids.includes(r.id));
        const copies = unm.copies.map((c) => c[0]), other = unm.other.map((c) => c[0]);
        const otherOn = rowsOf(other).filter((r) => r.enabled);
        return (
          <Module icon="warn" title="Unmanaged mods" count={unm.copies.length + unm.other.length + unm.frameworks.length}
            tools={<>
              <Key size="sm" variant="ghost" icon="eye" latched={from === "found"} onClick={() => setFrom(from === "found" ? "all" : "found")} title="List only unmanaged mods below">Show them</Key>
              <Key size="sm" variant="ghost" icon="clock" onClick={() => { setUnmLater(true); if (from === "found") setFrom("all"); toast("Moved to the bottom of the list"); }}
                title="Fold them into a group at the bottom. Nothing changes">Later</Key>
            </>}>
            <p className="dim" style={{ fontSize: ".9em" }}>Installed by neither Vortex nor Pockymod. Ctrl+Z undoes any cleanup</p>
            <div className="rows" style={{ marginTop: ".5em" }}>
              {unm.copies.length > 0 && (
                <div className="row" style={{ cursor: "default" }}>
                  <span className="m-main"><span className="m-t">{plural(unm.copies.length, "leftover copy", "leftover copies")} of Vortex mods</span>
                    <span className="m-meta"><span title={unm.copies.slice(0, 12).map((c) => `${c[1]} (copy of ${c[2]})`).join("\n")}>for example {unm.copies.slice(0, 3).map((c) => c[2]).join(", ")}</span><span>safe to remove, Vortex has its own</span></span></span>
                  <span className="m-side"><Key size="sm" icon="trash" disabled={busy} onClick={() => run(async () => { await ops.remove(rowsOf(copies)); toast(`Removed ${plural(copies.length, "leftover copy", "leftover copies")}`); })}
                    title="The Vortex originals stay">Remove {unm.copies.length}</Key></span>
                </div>
              )}
              {unm.frameworks.length > 0 && (
                <div className="row" style={{ cursor: "default" }}>
                  <span className="m-main"><span className="m-t">{plural(unm.frameworks.length, "framework")} put in by hand</span>
                    <span className="m-meta"><span>{unm.frameworks.map((f) => f[1]).join(", ")}</span><span className="okc">kept</span></span></span>
                </div>
              )}
              {unm.other.length > 0 && (
                <div className="row" style={{ cursor: "default" }}>
                  <span className="m-main"><span className="m-t">{plural(unm.other.length, "other mod")}</span>
                    <span className="m-meta"><span>{otherOn.length} on now</span><span>not in any Vortex profile</span></span></span>
                  <span className="m-side" style={{ flexDirection: "row", gap: ".3em" }}>
                    <Key size="sm" variant="ghost" icon="eye" onClick={() => { setFrom("found"); }} title="List them below">Review</Key>
                    {otherOn.length > 0 && <Key size="sm" icon="minus" disabled={busy} onClick={() => run(async () => { await ops.setEnabled(otherOn, false); toast(`Turned off ${plural(otherOn.length, "unmanaged mod")}`); })}
                      title="Nothing is deleted. Ctrl+Z turns them back on">Turn off {otherOn.length}</Key>}
                  </span>
                </div>
              )}
            </div>
          </Module>
        );
      })()}

      {vxp && (
        <Module icon="user" title="Your Vortex profiles" count={vxp.names.length}>
          <p>Vortex has {vxp.names.length} profiles ({vxp.names.join(", ")}). Bring them in as profiles here. The active one stays active</p>
          <div className="actions" style={{ marginTop: ".5em" }}>
            <Key icon="user" disabled={busy} onClick={() => run(async () => { const r = await vortexProfiles.import(); toast(profilesText(r)); setVxp(null); })}>Bring in {vxp.names.length} profiles</Key>
            <Key variant="ghost" title="Load order, Profiles has it later" onClick={() => { try { localStorage.setItem("pm.vortexProfiles.later", "1"); } catch { /* storage unavailable */ } setVxp(null); }}>Not now</Key>
          </div>
        </Module>
      )}

      {replacedRows.length > 0 && (
        <Module icon="warn" title="Your own files were set aside" count={replacedRows.length}>
          <p>Installing these set aside files you already had. Use mine puts yours back</p>
          {replacedRows.length > 1 && <div className="actions" style={{ marginTop: ".5em" }}><Key icon="check" disabled={busy} onClick={() => useMine(replacedRows.map((r) => r.id))}>Use mine for all {replacedRows.length}</Key></div>}
          <div className="rows" style={{ marginTop: ".5em" }}>
            {replacedRows.map((r) => (
              <div key={r.id} className="row" style={{ cursor: "default" }}>
                <span className="m-main"><span className="m-t">{r.name}</span>
                  <span className="m-meta"><span>{r.replaced} {r.replaced === 1 ? "file" : "files"} of yours set aside</span>{r.origin && <span>installed for {r.origin}</span>}</span></span>
                <span className="m-side" style={{ flexDirection: "row", gap: ".3em" }}><Key size="sm" disabled={busy} onClick={() => useMine([r.id])}>Use mine</Key></span>
              </div>
            ))}
          </div>
        </Module>
      )}

      {dupes.length > 0 && (
        <Module icon="library" title="Mods you have twice" count={dupes.length}
          tools={<Key size="sm" variant="ghost" icon={showExtra ? "hide" : "eye"} onClick={() => setShowExtra(!showExtra)}>{showExtra ? "Hide extra copies" : "Show extra copies"}</Key>}>
          {dupes.length > 1 && <div className="actions" style={{ marginTop: ".5em" }}><Key icon="check" disabled={busy} onClick={keepAll}>Tidy up all {dupes.length}</Key></div>}
          <div className="rows" style={{ marginTop: ".5em" }}>
            {dupes.map((d) => {
              const keep = byId.get(d[0]);
              const others = d.slice(1).map((id) => byId.get(id)).filter((x): x is InstallRow => !!x);
              if (!keep) return null;
              const tag = (r: InstallRow) => `${r.version ? `v${r.version}` : "version unknown"}, ${r.source === "pocket" ? (r.origin ? `added for ${r.origin}` : "Pockymod") : SOURCE[r.source]}${r.enabled ? ", on" : ", off"}`;
              return (
                <div key={d[0]} className="row" style={{ cursor: "default" }}>
                  <span className="m-main"><span className="m-t">{keep.name}</span>
                    <span className="m-meta"><span className="okc">keep: {tag(keep)}</span>{others.map((o) => <span key={o.id}>extra: {tag(o)}</span>)}</span></span>
                  <span className="m-side" style={{ flexDirection: "row", gap: ".3em" }}><Key size="sm" disabled={busy} onClick={() => keepOne(d)}>Keep this one</Key></span>
                </div>
              );
            })}
          </div>
        </Module>
      )}

      {rows?.length === 0 && (
        <Module icon="library" title="Your mods">
          <Empty icon="library" title="No mods yet" actions={<Key icon="folder" onClick={() => onExisting?.()}>Find mods I already have</Key>}>
            Install from Browse or bring in what you already have
          </Empty>
        </Module>
      )}

      <div className="lib-groups">
      {groupNames.map((g) => {
        const list = byGroup.get(g)!;
        const ids = list.map((r) => r.id);
        const allSel = ids.every((i) => sel.has(i));
        const later = g === LATER;
        // the set-aside group starts folded
        const isFolded = later ? !folded.has(g) : folded.has(g);
        // small groups sit side by side on wide windows instead of leaving a row mostly empty
        const basis = layout === "grid" ? `${Math.max(2, Math.min(list.length, 6)) * 13.3 + 1.4}em` : "46em";
        return (
          <div key={g} className="lib-group" style={{ flexBasis: basis }}>
          <Module icon={later ? "clock" : g === "Core" ? "chip" : g === "Character presets" ? "user" : "library"} title={g}
            count={`${list.filter((r) => r.enabled).length} of ${list.length} on`} bodyClass="list"
            tools={<>
              {later && <Key size="sm" variant="ghost" icon="warn" onClick={() => setUnmLater(false)} title="Back to Unmanaged mods at the top">Sort them now</Key>}
              <Key size="sm" variant="ghost" onClick={() => toggleAll(ids)}>{allSel ? "Unselect" : "Select all"}</Key>
              <Key size="sm" variant="ghost" icon="chevron" className={isFolded ? "" : "rot-key"} aria-label={isFolded ? "Open" : "Fold"}
                onClick={() => setFolded((f) => { const n = new Set(f); n.has(g) ? n.delete(g) : n.add(g); return n; })} />
            </>}>
            {isFolded && later && (
              <button type="button" className="linkish fold-line" onClick={() => setFolded((f) => new Set(f).add(g))}>
                {plural(list.length, "unmanaged mod")}
              </button>
            )}
            {!isFolded && layout === "grid" && (
              <div className="lgrid2">
                {list.map((r) => (
                  <div key={r.id} className={`lcard${r.enabled ? "" : " off"}${sel.has(r.id) ? " sel" : ""}`} role="button" tabIndex={0} aria-pressed={sel.has(r.id)}
                    onClick={() => toggle(r.id)} onKeyDown={(e) => { if (e.key === " " || e.key === "Enter") { e.preventDefault(); toggle(r.id); } }}>
                    <span className="lcard-img">
                      {r.picture ? <img src={r.picture.replace("/images/", "/images/thumbnails/")} alt="" loading="lazy" onError={(e) => { const i = e.target as HTMLImageElement; if (r.picture && i.src !== r.picture) i.src = r.picture; }} /> : <Icon name={r.modType === "framework" ? "chip" : "file"} size={27} />}
                      <input type="checkbox" className="pick" checked={sel.has(r.id)} onChange={() => toggle(r.id)} onClick={(e) => e.stopPropagation()} aria-label={`Select ${r.name}`} />
                      {updOf(r) && <span className="lcard-badge"><Pill tone="soon">Update</Pill></span>}
                    </span>
                    <span className="lcard-b">
                      <span className="m-t">{r.name}</span>
                      <span className="m-meta">{r.version && <span>v{r.version}</span>}{SOURCE[r.source] && <span>{SOURCE[r.source]}</span>}</span>
                      <span className="lcard-f" onClick={(e) => e.stopPropagation()}>
                        <Switch checked={r.enabled} onChange={(v) => doBulk(v ? "enable" : "disable", [r.id])} disabled={busy}>{r.enabled ? "On" : "Off"}</Switch>
                        {updOf(r) && r.modId < 900_000_000 && <Key size="sm" icon="downloads" onClick={() => run(async () => { await queueUpdate(r); toast(`${r.name} update queued`); })} disabled={busy} aria-label={`Update ${r.name}`} />}
                        {r.hasOptions && <Key size="sm" variant="ghost" icon="tool" onClick={() => changeOptions(r)} disabled={busy} aria-label={`Change the options of ${r.name}`} title="Change installer options" />}
                        {star(r)}
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
                        <span>{r.deployedFiles} {r.deployedFiles === 1 ? "file" : "files"} in the game</span>
                        {r.category && <span>{r.category}</span>}
                        {SOURCE[r.source] && <span><Pill tone={r.source === "vortex" ? "soon" : undefined}>{SOURCE[r.source]}</Pill></span>}
                        {updOf(r) && <span><Pill tone="soon">Update {updOf(r)!.latest}</Pill></span>}
                      </span>
                      <span className="m-tags">
                        <Tag kind={r.modType === "unknown" || r.modType === "mixed" ? undefined : r.modType} square>{r.modType === "redmod" ? "REDmod" : r.modType}</Tag>
                        {r.frameworks.map((f) => <Tag key={f} kind={f}>{FRAMEWORKS.find((x) => x.key === f)?.short ?? f}</Tag>)}
                      </span>
                    </span>
                    <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".4em" }}>
                      {updOf(r) && r.modId < 900_000_000 && <Key size="sm" icon="downloads" onClick={() => run(async () => { await queueUpdate(r); toast(`${r.name} update queued`); })} disabled={busy}>Update</Key>}
                      {updOf(r) && <Key size="sm" variant="ghost" onClick={() => run(async () => { const u = updOf(r)!; await updatesApi.skip(u.modId, u.latest); toast(`Skipping ${r.name} ${u.latest}`); })} disabled={busy} title="Skip this version">Skip</Key>}
                      {r.enabled && r.deployedFiles === 0 && <Key size="sm" onClick={() => deploy(r)} disabled={busy}>Deploy</Key>}
                      {r.hasOptions && <Key size="sm" variant="ghost" icon="tool" onClick={() => changeOptions(r)} disabled={busy} title="Pick other installer options">Change options</Key>}
                      {star(r)}
                      <Switch checked={r.enabled} onChange={(v) => doBulk(v ? "enable" : "disable", [r.id])} disabled={busy}><span className="sr-only">On</span></Switch>
                      <Key size="sm" variant="ghost" icon="trash" onClick={() => doBulk("uninstall", [r.id])} disabled={busy} aria-label={`Remove ${r.name}`} title={r.source === "vortex" ? "Remove from the game. Stays in Vortex" : "Remove"} />
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
              {trash.slice(0, 3).map((t) => t.name).join(", ")}{trash.length > 3 ? ` and ${trash.length - 3} more` : ""}, kept 14 days
            </button>
          ) : (
            <>
              <div className="rows">
                {trash.map((t) => (
                  <div key={t.id} className="row" style={{ cursor: "default" }}>
                    <span className="m-main"><span className="m-t">{t.name}</span>
                      <span className="m-meta">{t.version && <span>v{t.version}</span>}<span>{t.reason === "updated" ? "replaced by an update" : t.reason}</span><span>{ago(t.removedAt)}</span>{t.size > 0 && <span>{bytesText(t.size)}</span>}{t.source === "vortex" && <span>still in Vortex</span>}</span></span>
                    <span className="m-side" style={{ flexDirection: "row", gap: ".3em" }}>
                      <Key size="sm" icon="undo" disabled={busy} onClick={() => run(async () => { await ops.restoreRemoved([t.id], [t.name]); })}>Put back</Key>
                      <Key size="sm" variant="ghost" icon="x" disabled={busy} aria-label={`Delete ${t.name} for good`} title="Delete for good" onClick={() => run(async () => { await library.purge(t.id); })} />
                    </span>
                  </div>
                ))}
              </div>
              <div className="actions" style={{ justifyContent: "flex-end", padding: ".5em .8em" }}>
                <Key size="sm" variant="ghost" icon="trash" disabled={busy} onClick={() => run(async () => { const n = await library.purge(undefined, true); toast(`Deleted ${n} ${n === 1 ? "mod" : "mods"} for good`); })}>Empty</Key>
              </div>
            </>
          )}
        </Module>
      )}

      {choices && <FomodDialog need={choices} onClose={() => setChoices(null)} onInstalled={load} />}
      {sortPlan && <SortPlan plan={sortPlan} busy={busy} onClose={() => setSortPlan(null)} onApply={() => run(async () => {
        const by = new Map<string, number[]>();
        for (const p of sortPlan) by.set(p.group, [...(by.get(p.group) ?? []), p.row.id]);
        for (const [g, ids] of by) await groupsApi.set(ids, g);
        setSortPlan(null);
        toast(`${sortPlan.length} ${sortPlan.length === 1 ? "mod" : "mods"} sorted into ${by.size} ${by.size === 1 ? "group" : "groups"}`);
      })} />}
      {reopt && <FomodDialog need={reopt.need} onClose={() => setReopt(null)} apply={(picks) => applyOptions(reopt.row, picks, reopt.before)} />}
      {moveOpen && (
        <Dialog icon="folder" title="Move to group" count={plural(sel.size, "mod")} onClose={() => setMoveOpen(false)}>
          <div className="chips">
            {customGroups.map((g) => <button key={g} type="button" className="chip" onClick={() => moveTo(g)}>{g}</button>)}
            <button type="button" className="chip" onClick={() => moveTo(null)}>Back to their own group</button>
          </div>
          <div className="actions" style={{ flexWrap: "nowrap" }}>
            <label className="fbox"><input className="finput" value={newGroup} onChange={(e) => setNewGroup(e.target.value)} placeholder="New group" aria-label="New group" onKeyDown={(e) => { if (e.key === "Enter" && newGroup.trim()) moveTo(newGroup); }} /></label>
            <Key icon="plus" disabled={!newGroup.trim() || busy} onClick={() => moveTo(newGroup)}>Make and move</Key>
          </div>
        </Dialog>
      )}
      {pending?.kind === "conflicts" && (
        <Dialog icon="warn" title="Files already in use" count={plural(pending.conflicts.length, "file")} band="bad" onClose={() => setPending(null)}>
          <p>{pending.name} needs files other mods already put in the game</p>
          <div className="log" style={{ maxHeight: "10em" }}>{pending.conflicts.slice(0, 50).map((c) => `${c.target}  ·  ${c.ownerName}`).join("\n")}</div>
          <div className="actions" style={{ justifyContent: "flex-end" }}>
            <Key variant="ghost" onClick={() => setPending(null)}>Leave it staged</Key>
            <Key onClick={() => deploy({ id: pending.installId, name: pending.name }, true)} disabled={busy} title="The other mods get them back if you remove it">Take them over</Key>
          </div>
        </Dialog>
      )}
      {pending?.kind === "copy" && (
        <Dialog icon="warn" title="Can't hard link" onClose={() => setPending(null)}>
          <p>{pending.reason}</p>
          <div className="note warn">Copying uses the space twice. Put the mod library on the game's drive in <SettingsLink section="Game and mod folders" /></div>
          <div className="actions" style={{ justifyContent: "flex-end" }}>
            <Key variant="ghost" onClick={() => setPending(null)}>Cancel</Key>
            <Key onClick={() => deploy({ id: pending.installId, name: pending.name }, false, true)} disabled={busy}>Copy this once</Key>
          </div>
        </Dialog>
      )}
      {pending?.kind === "review" && (
        <Dialog icon="warn" title="Unknown files" count={plural(pending.plan.unknown.length, "file")} onClose={() => setPending(null)}>
          <p>{pending.download.name} has files with no known place. They'll be left out</p>
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

/** Where the extensions' group rules would put your mods, before anything moves. */
function SortPlan({ plan, busy, onClose, onApply }: { plan: ReturnType<typeof planGroups>; busy: boolean; onClose: () => void; onApply: () => void }) {
  return (
    <Dialog icon="folder" title="Sort into groups" count={`${plan.length} ${plan.length === 1 ? "mod" : "mods"}`} onClose={onClose}>
      {plan.length === 0 ? <p>Everything is already sorted</p> : <>
        <p>Mods in your own groups stay put</p>
        <div className="rows sortplan">
          {plan.map((p) => (
            <div key={p.row.id} className="row">
              <span className="m-main"><span className="m-t">{p.row.name}</span><span className="m-meta"><span>{p.row.group} → {p.group}</span>{p.from && <span>{p.from}</span>}</span></span>
            </div>
          ))}
        </div>
      </>}
      <div className="actions" style={{ justifyContent: "flex-end" }}>
        <Key variant="ghost" onClick={onClose}>{plan.length ? "Cancel" : "Close"}</Key>
        {plan.length > 0 && <Key icon="folder" disabled={busy} onClick={onApply}>Move {plan.length} {plan.length === 1 ? "mod" : "mods"}</Key>}
      </div>
    </Dialog>
  );
}
