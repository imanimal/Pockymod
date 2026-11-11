// What a mod's card and page can do without a trip to Library. Not installed: Quick install takes the easiest way in
// (the newest main file and its requirements, straight to the queue; Premium downloads at once, a free account gets
// one click on Nexus). Installed: a menu with everything the app can do to it, the same actions as Library (on or
// off, update or skip, deploy, change options, group, every profile, remove) plus Nexus's (endorse, track, open the
// page in the app or in your own browser) and Browse's (hide, ignore). Changes go through lib/ops, so Ctrl+Z works.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Icon } from "../../ui/Icon";
import { Key, Switch } from "../../ui/pocket";
import { Popover } from "../../ui/Popover";
import { toast } from "../../ui/toast";
import {
  browse, endorseMod, errorText, groups as groupsApi, installerOptions, library, nexusPages, nexusUrl, openExternal, profiles, queue, queueMod, trackMod,
  updates as updatesApi, type ChoicesNeeded, type InstallRow, type Membership, type ModFile, type QueueSnapshot, type Update,
} from "../../lib/api";
import { installPrefs } from "../../lib/prefs";
import { held } from "../../lib/tauri";
import { record } from "../../lib/history";
import * as ops from "../../lib/ops";
import { FomodDialog } from "../FomodDialog";

/** The file Install picks: the newest main file, else the newest current one. */
export function mainFile(files: ModFile[]): ModFile | undefined {
  const by = (cats: string[]) => files.filter((f) => cats.includes(f.category)).sort((a, b) => b.date - a.date)[0];
  return by(["MAIN"]) ?? by(["MAIN", "OPTIONAL", "UPDATE", "MISCELLANEOUS"]);
}

/** Queue a mod the easiest way: its main file, with the requirements the Install settings ask for, no plan dialog. */
export async function quickInstall(modId: number, name: string): Promise<boolean> {
  try {
    const [p, d] = await Promise.all([installPrefs().catch(() => ({ requirements: "always" as const, confirm: true })), browse.detail(modId)]);
    const f = mainFile(d.files);
    if (!f) { toast(`${name} has no current file on Nexus`, true); return false; }
    const snap = await queue.add(modId, f.fileId, p.requirements !== "never");
    const mine = snap.items.filter((i) => i.state.state === "waiting" || i.state.state === "needsClick" || i.state.state === "downloading");
    const extra = mine.filter((i) => i.requiredBy === d.detail.name).length;
    toast(`${name} is queued${extra ? ` with ${extra} required ${extra === 1 ? "mod" : "mods"}` : ""}`);
    return true;
  } catch (e) { toast(errorText(e), true); return false; }
}

/** Every installed mod by Nexus mod id, and the queue, kept current. */
export function useInstalled() {
  const [rows, setRows] = useState<InstallRow[]>([]);
  const [snap, setSnap] = useState<QueueSnapshot | null>(null);
  useEffect(() => {
    const load = () => library.list().then(setRows).catch(() => {});
    load();
    queue.get().then(setSnap).catch(() => {});
    const offs: Array<() => void> = [];
    offs.push(held(library.onChanged(load)), held(queue.onChange(setSnap)));
    return () => offs.forEach((f) => f());
  }, []);
  const byMod = useMemo(() => { const m = new Map<number, InstallRow[]>(); for (const r of rows) m.set(r.modId, [...(m.get(r.modId) ?? []), r]); return m; }, [rows]);
  const queued = useMemo(() => {
    const m = new Map<number, string>();
    for (const i of snap?.items ?? []) {
      const s = i.state.state;
      if (s === "waiting" || s === "needsClick" || s === "downloading" || s === "installing") m.set(i.modId, s === "needsClick" ? "Needs your click" : s === "downloading" ? "Downloading" : s === "installing" ? "Installing" : "Queued");
    }
    return m;
  }, [snap]);
  return { byMod, queued };
}

export interface ModRef { modId: number; name: string; version?: string }

/** The key on a card or page: Quick install, the queue's state, or the menu of an installed mod. */
export function ModKey({ mod, rows, queued, updates, onDetail, onLibrary, onFlag, flags, size = "sm", label }: {
  mod: ModRef; rows: InstallRow[]; queued?: string; updates?: Update[]; onDetail?: () => void; onLibrary?: () => void;
  onFlag?: (flag: "hidden" | "ignored", on: boolean) => void; flags?: { hidden: boolean; ignored: boolean }; size?: "sm" | "lg"; label?: boolean;
}) {
  const ref = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [reopt, setReopt] = useState<{ row: InstallRow; need: ChoicesNeeded; before: ChoicesNeeded["defaults"] } | null>(null);
  const stop = (e: React.SyntheticEvent) => e.stopPropagation();
  const close = useCallback(() => setOpen(false), []);
  if (!rows.length) {
    if (queued) return <span className="modkey" onClick={stop}><Key size={size} variant="ghost" icon="clock" disabled title={`${mod.name}: ${queued.toLowerCase()} (Downloads)`}>{queued}</Key></span>;
    return (
      <span className="modkey" onClick={stop} onKeyDown={stop}>
        <Key size={size} icon="install" busy={busy} aria-label={`Quick install ${mod.name}`}
          title="Queue the newest main file and its requirements"
          onClick={async () => { setBusy(true); await quickInstall(mod.modId, mod.name); setBusy(false); }}>{label ? "Quick install" : "Install"}</Key>
      </span>
    );
  }
  const on = rows.some((r) => r.enabled);
  const upd = updates?.some((u) => u.modId === mod.modId);
  return (
    <span className="modkey" ref={ref} onClick={stop} onKeyDown={stop}>
      <Key size={size} variant={upd ? undefined : "dark"} icon={upd ? "downloads" : on ? "check" : "minus"} latched={open} busy={busy} aria-haspopup="menu" aria-expanded={open}
        aria-label={`${mod.name}: installed, ${on ? "on" : "off"}. Options`} title="On/off, update, group, profiles, remove"
        onClick={() => setOpen(!open)}>{upd ? "Update" : on ? "Installed" : "Off"}<Icon name="down" size={9} /></Key>
      {open && (
        <Popover anchor={ref.current} onClose={close} label={`${mod.name} options`}>
          <ManageMenu mod={mod} rows={rows} updates={updates ?? []} onClose={close} onDetail={onDetail} onLibrary={onLibrary} onFlag={onFlag} flags={flags}
            onOptions={(r) => { setOpen(false); void changeOptions(r, setReopt, setBusy); }} />
        </Popover>
      )}
      {reopt && createPortal(<FomodDialog need={reopt.need} onClose={() => setReopt(null)} apply={(picks) => applyOptions(reopt.row, picks, reopt.before)} />, document.querySelector(".shell") ?? document.body)}
    </span>
  );
}

async function changeOptions(r: InstallRow, set: (v: { row: InstallRow; need: ChoicesNeeded; before: ChoicesNeeded["defaults"] }) => void, busy: (b: boolean) => void) {
  busy(true);
  try {
    const o = await installerOptions.get(r.id);
    if (!o.saved) toast(`No saved picks for ${r.name}, showing the installer defaults`);
    set({ row: r, need: { request: { modId: r.modId, fileId: r.fileId, name: r.name, version: r.version, archive: "" }, fomod: o.fomod, defaults: o.choices }, before: o.choices });
  } catch (e) { toast(errorText(e), true); } finally { busy(false); }
}
async function applyOptions(r: InstallRow, picks: ChoicesNeeded["defaults"], before: ChoicesNeeded["defaults"]) {
  const d = await installerOptions.change(r.id, picks);
  if (d && d.outcome !== "deployed") toast(`${r.name}'s new options conflict with other mods. Sort it out in Library`, true);
  record({ label: `Changed the options of ${r.name}`, undo: () => installerOptions.change(r.id, before), redo: () => installerOptions.change(r.id, picks) }, !!d && d.outcome !== "deployed");
  if (!d) toast(`Options saved. They apply when ${r.name} is turned on`);
}

const SOURCE: Record<InstallRow["source"], string> = { pocket: "Pockymod", vortex: "Vortex", found: "found in the game" };

/** Everything the app can do to an installed mod. */
export function ManageMenu({ mod, rows, updates, onClose, onDetail, onLibrary, onFlag, flags, onOptions }: {
  mod: ModRef; rows: InstallRow[]; updates: Update[]; onClose: () => void; onDetail?: () => void; onLibrary?: () => void;
  onFlag?: (flag: "hidden" | "ignored", on: boolean) => void; flags?: { hidden: boolean; ignored: boolean }; onOptions: (r: InstallRow) => void;
}) {
  // which key's work is running: that key shows it, the others wait
  const [busy, setBusy] = useState<string | null>(null);
  const working = busy !== null;
  const [groupList, setGroupList] = useState<{ presets: string[]; custom: string[] }>({ presets: [], custom: [] });
  const [member, setMember] = useState<Map<number, Membership[]>>(new Map());
  const [newGroup, setNewGroup] = useState("");
  const load = useCallback(() => {
    groupsApi.list().then(setGroupList).catch(() => {});
    Promise.all(rows.map((r) => profiles.memberships(r.id).then((m) => [r.id, m] as const))).then((l) => setMember(new Map(l))).catch(() => {});
  }, [rows]);
  useEffect(() => { load(); }, [load]);
  const run = async (id: string, f: () => Promise<unknown>, closeAfter = false) => {
    setBusy(id);
    try { await f(); } catch (e) { toast(errorText(e), true); } finally { setBusy(null); load(); if (closeAfter) onClose(); }
  };
  const updOf = (r: InstallRow) => updates.find((u) => u.installId === r.id) ?? updates.find((u) => !u.installId && u.modId === r.modId);
  const nexus = mod.modId < 900_000_000;
  const row0 = rows[0];
  const custom = row0.customGroup ? row0.group : null;
  const setInProfile = (m: Membership, r: InstallRow, v: boolean) => run("profile", async () => {
    if (m.active) { await ops.setEnabled([r], v); return; }
    await profiles.setIn(m.profileId, r.id, v);
    record({ label: `Turned ${v ? "on" : "off"} ${r.name} in ${m.name}`, undo: () => profiles.setIn(m.profileId, r.id, !v), redo: () => profiles.setIn(m.profileId, r.id, v) });
  });
  const addGroup = () => {
    const g = newGroup.trim();
    if (!g || working) return;
    void run("newGroup", async () => { await groupsApi.create(g); await ops.moveToGroup(rows, g); setNewGroup(""); });
  };

  return (
    <>
      <div className="ph"><span className="pt"><Icon name="library" />{mod.name}</span><span className="cnt">{rows.length === 1 ? "installed" : `${rows.length} files`}</span></div>
      <div className="pb modmenu">
        {rows.map((r) => {
          const u = updOf(r);
          return (
            <div key={r.id} className="mm-file">
              <div className="mm-line">
                <span className="m-main"><span className="m-t">{rows.length > 1 ? (r.name !== mod.name ? r.name : `File ${r.fileId}`) : "This copy"}</span>
                  <span className="m-meta">{r.version && <span>v{r.version}</span>}<span>{SOURCE[r.source]}</span><span>{r.deployedFiles} {r.deployedFiles === 1 ? "file" : "files"} in the game</span></span></span>
                <Switch checked={r.enabled} disabled={working} onChange={(v) => run("switch", () => ops.setEnabled([r], v))}>{r.enabled ? "On" : "Off"}</Switch>
              </div>
              <div className="actions">
                {u && nexus && <Key size="sm" icon="downloads" busy={busy === `update${r.id}`} disabled={working} onClick={() => run(`update${r.id}`, async () => { await (u.fileId ? queue.add(u.modId, u.fileId, true) : queueMod(r.modId)); toast(`${r.name} ${u.latest} queued`); }, true)}>Update to {u.latest}</Key>}
                {u && <Key size="sm" variant="ghost" busy={busy === `skip${r.id}`} disabled={working} title="Do not offer this version again" onClick={() => run(`skip${r.id}`, async () => { await updatesApi.skip(u.modId, u.latest); toast(`Skipped ${r.name} ${u.latest}`); })}>Skip {u.latest}</Key>}
                {r.enabled && r.deployedFiles === 0 && <Key size="sm" icon="install" busy={busy === `deploy${r.id}`} disabled={working} onClick={() => run(`deploy${r.id}`, async () => {
                  const d = await library.deploy(r.id, { takeConflicts: false, allowCopy: false });
                  if (d.outcome === "deployed") toast(`Deployed ${r.name}`); else { toast(`${r.name} ${d.outcome === "conflicts" ? "conflicts with other mods" : "needs copying"}. Sort it out in Library`, true); onLibrary?.(); }
                })}>Deploy</Key>}
                {r.hasOptions && <Key size="sm" variant="ghost" icon="tool" disabled={working} onClick={() => onOptions(r)} title="Pick other installer options, no download">Change options</Key>}
                <Key size="sm" variant="ghost" icon="trash" busy={busy === `remove${r.id}`} disabled={working} title={r.source === "vortex" ? "Remove from the game, keep in Vortex" : "Goes to Recently removed"} onClick={() => run(`remove${r.id}`, () => ops.remove([r]), true)}>Remove</Key>
              </div>
              {(member.get(r.id)?.length ?? 0) > 1 && (
                <div className="mm-sec">
                  <span className="sub-h">Profiles</span>
                  {member.get(r.id)!.map((m) => (
                    <Switch key={m.profileId} checked={m.enabled} disabled={working} onChange={(v) => setInProfile(m, r, v)}>
                      {m.name}{m.active ? <span className="dim"> · active</span> : null}
                    </Switch>
                  ))}
                </div>
              )}
            </div>
          );
        })}
        <div className="mm-sec">
          <span className="sub-h">Group</span>
          <select value={custom ?? ""} disabled={working} aria-label="Group" onChange={(e) => run("group", () => ops.moveToGroup(rows, e.target.value || null))}>
            <option value="">{custom ? "Default group" : `${row0.group} (default)`}</option>
            {groupList.custom.map((g) => <option key={g} value={g}>{g}</option>)}
          </select>
          <div className="fbox" style={{ flex: "none" }}>
            <input className="finput" value={newGroup} onChange={(e) => setNewGroup(e.target.value)} placeholder="New group" aria-label="New group name"
              onKeyDown={(e) => { if (e.key === "Enter") addGroup(); }} />
            <Key size="sm" variant="ghost" icon="plus" busy={busy === "newGroup"} disabled={working || !newGroup.trim()} aria-label="Create group and move here"
              title="Make this group and move the mod into it (Enter)" onClick={addGroup} />
          </div>
        </div>
        <div className="mm-sec">
          <span className="sub-h">More</span>
          <div className="actions">
            {onDetail && <Key size="sm" variant="ghost" icon="info" onClick={() => { onClose(); onDetail(); }}>Page and other files</Key>}
            {onLibrary && <Key size="sm" variant="ghost" icon="library" onClick={() => { onClose(); onLibrary(); }}>Show in Library</Key>}
            {nexus && <Key size="sm" variant="ghost" icon="star" busy={busy === "endorse"} disabled={working} onClick={() => run("endorse", async () => { await endorseMod(mod.modId, mod.version ?? row0.version); toast(`Endorsed ${mod.name}`); })}>Endorse</Key>}
            {nexus && <Key size="sm" variant="ghost" icon="eye" busy={busy === "track"} disabled={working} onClick={() => run("track", async () => { await trackMod(mod.modId); toast(`Tracking ${mod.name}`); })}>Track</Key>}
            {nexus && <Key size="sm" variant="ghost" icon="web" busy={busy === "page"} onClick={() => run("page", () => nexusPages.mod(mod.modId), true)} title="Opens where Settings says Nexus pages go">Nexus page</Key>}
            {nexus && <Key size="sm" variant="ghost" icon="link" busy={busy === "browser"} onClick={() => run("browser", () => openExternal(nexusUrl.mod(mod.modId)), true)} title="Open in your browser">Open in browser</Key>}
            {onFlag && flags && <Key size="sm" variant="ghost" icon="hide" onClick={() => { onFlag("hidden", !flags.hidden); onClose(); }}>{flags.hidden ? "Unhide in Browse" : "Hide in Browse"}</Key>}
            {onFlag && flags && <Key size="sm" variant="ghost" icon="x" onClick={() => { onFlag("ignored", !flags.ignored); onClose(); }}>{flags.ignored ? "Stop ignoring" : "Ignore"}</Key>}
          </div>
        </div>
      </div>
    </>
  );
}
