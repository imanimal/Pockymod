// Library: installed mods for the active profile, with profile switching. The orange key installs updates
// when there are any.

import { useEffect, useMemo, useState } from "react";
import { Deck } from "../components/Deck";
import { Chip, Empty, Grp, Key, Lcd, Module, Pill, SearchField, Switch, Thumb, cx } from "../components/pocket";
import { api, pickFile } from "../lib/api";
import { bytes, date } from "../lib/format";
import { useAction, useApp } from "../lib/store";
import { KIND_NAMES, type LibraryRow, type ModKind } from "../lib/types";
import { FrameworkTags, KindTag } from "./Browse";

export function Library() {
  const { libraryRev, refresh, toast, go } = useApp();
  const [rows, setRows] = useState<LibraryRow[]>([]);
  const [profiles, setProfiles] = useState<{ profiles: string[]; active: string }>({ profiles: [], active: "" });
  const [sel, setSel] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [kind, setKind] = useState<ModKind | null>(null);
  const [newProfile, setNewProfile] = useState("");
  const { busy, run } = useAction();

  const reload = () => {
    api.library().then(setRows).catch(() => {});
    api.profiles().then(setProfiles).catch(() => {});
  };
  useEffect(reload, [libraryRev]);

  const shown = useMemo(
    () => rows.filter((r) => (!kind || r.kind === kind) && (!filter || r.name.toLowerCase().includes(filter.toLowerCase()))),
    [rows, kind, filter],
  );
  const updates = rows.filter((r) => r.updateAvailable);
  const current = rows.find((r) => r.key === sel) ?? null;
  const enabled = rows.filter((r) => r.enabled).length;
  const kinds = [...new Set(rows.map((r) => r.kind))];

  const installFile = async () => {
    const p = await pickFile("Install a mod archive", ["zip", "7z", "rar"]);
    if (!p) return;
    const r = await run("file", () => api.installFile(p));
    if (r) {
      toast(`Installed ${r.name}.`);
      reload();
      refresh();
    }
  };

  return (
    <div className="library">
      <Deck>
        <Key icon="refresh" disabled={busy === "check"} onClick={() => run("check", async () => { const n = await api.checkUpdates(); toast(n ? `${n} update(s) found.` : "Everything is up to date."); reload(); refresh(); })}>
          Check for updates
        </Key>
        <Key icon="file" onClick={installFile}>
          Install from file
        </Key>
        <Key icon="collections" disabled={rows.length === 0} onClick={() => go("collections", { exportKeys: rows.filter((r) => r.enabled).map((r) => r.key) })}>
          Share as a collection
        </Key>
      </Deck>

      <Module title="Profile" icon="user" count={profiles.active} auto className="lib-profile">
        <div className="lcds">
          <Lcd label="Mods" value={rows.length} sub={`${enabled} enabled`} size="sm" />
          <Lcd label="Updates" value={updates.length} tone={updates.length ? "hot" : null} size="sm" sub={updates.length ? "ready to install" : "none waiting"} />
        </div>
        <div className="chips">
          {profiles.profiles.map((p) => (
            <Chip key={p} on={p === profiles.active} onClick={() => p !== profiles.active && run("switch", async () => { await api.switchProfile(p); reload(); refresh(); toast(`Switched to ${p}.`); })}>
              {p}
            </Chip>
          ))}
        </div>
        <div className="inline-form">
          <input value={newProfile} placeholder="New profile name" aria-label="New profile name" onChange={(e) => setNewProfile(e.target.value)} />
          <Key size="sm" icon="plus" disabled={!newProfile.trim()} onClick={() => run("new", async () => { await api.createProfile(newProfile, true); setNewProfile(""); reload(); })}>
            Copy current
          </Key>
        </div>
        {updates.length ? (
          <div className="actions">
            <Key variant="primary" icon="download" disabled={busy === "upd"} onClick={() => run("upd", async () => { const n = await api.queueUpdates(); toast(`${n} update(s) queued.`, { action: { label: "Open queue", run: () => go("downloads") } }); })}>
              Install {updates.length} update{updates.length === 1 ? "" : "s"}
            </Key>
          </div>
        ) : null}
        {profiles.profiles.filter((p) => p !== profiles.active && p !== "Default").length ? (
          <div className="chips">
            {profiles.profiles
              .filter((p) => p !== profiles.active && p !== "Default")
              .map((p) => (
                <Key key={p} size="sm" variant="ghost" icon="trash" onClick={() => run("del", async () => { await api.deleteProfile(p); reload(); })}>
                  Delete {p}
                </Key>
              ))}
          </div>
        ) : null}
      </Module>

      <Module title="Installed" icon="library" count={shown.length} className="lib-list" bodyClassName="list-body">
        <div className="fbar">
          <SearchField value={filter} onChange={setFilter} placeholder="Filter installed mods" />
        </div>
        <div className="qstrip">
          <Chip on={!kind} onClick={() => setKind(null)} count={rows.length}>
            All
          </Chip>
          {kinds.map((k) => (
            <Chip key={k} on={kind === k} onClick={() => setKind(kind === k ? null : k)} count={rows.filter((r) => r.kind === k).length}>
              {KIND_NAMES[k]}
            </Chip>
          ))}
        </div>
        {rows.length === 0 ? (
          <Empty icon="library" title="Nothing installed yet" actions={<Key icon="browse" onClick={() => go("browse")}>Browse mods</Key>} />
        ) : (
          <div className="rows" role="listbox" aria-label="Installed mods">
            <Grp count={`${enabled} on`}>{profiles.active}</Grp>
            {shown.map((r) => (
              <div key={r.key} className={cx("row mrow lrow", sel === r.key && "sel", !r.enabled && "off")} role="option" aria-selected={sel === r.key} tabIndex={0} onClick={() => setSel(r.key)}>
                <Thumb src={r.thumbnail} alt="" />
                <div className="m-main">
                  <span className="m-t">{r.name}</span>
                  <span className="m-meta">
                    <span>{r.version ? `v${r.version}` : "no version"}</span>
                    <span>{bytes(r.size)}</span>
                    <span>installed {date(r.installedAt)}</span>
                  </span>
                  <span className="m-tags">
                    <KindTag kind={r.kind} />
                    <FrameworkTags list={r.frameworks} />
                    {r.updateAvailable ? <Pill tone="soon">Update {r.updateAvailable.version ?? ""}</Pill> : null}
                    {r.drift ? <Pill tone="over">{r.drift} file(s) changed</Pill> : null}
                  </span>
                </div>
                <span />
                <div className="m-due" onClick={(e) => e.stopPropagation()}>
                  <Switch
                    checked={r.enabled}
                    label={`${r.enabled ? "Disable" : "Enable"} ${r.name}`}
                    onChange={(v) => run("toggle", async () => { await api.setEnabled(r.key, v); reload(); refresh(); })}
                  />
                </div>
              </div>
            ))}
          </div>
        )}
      </Module>

      <Module title="Selected" icon="chip" className="lib-detail" count={current ? current.key : null}>
        {!current ? (
          <Empty icon="chip" title="Pick a mod">Its files, frameworks and keys show here.</Empty>
        ) : (
          <div className="detail-body">
            <h2 className="d-title">{current.name}</h2>
            <div className="meta-row">
              <span>{current.author ?? "Unknown author"}</span>
              <span className="dim">· {current.fileName}</span>
            </div>
            <div className="lcds">
              <Lcd label="Files" value={current.files} size="sm" sub={current.deployed ? "in the game folder" : "not deployed"} />
              <Lcd label="Changed" value={current.drift} size="sm" tone={current.drift ? "bad" : null} sub="outside the app" />
            </div>
            {current.unplaced.length ? (
              <>
                <div className="sect">Left out at install</div>
                <ul className="files">
                  {current.unplaced.map((f) => (
                    <li key={f} className="mono">{f}</li>
                  ))}
                </ul>
              </>
            ) : null}
            {current.redmods.length ? <p className="note">Adds REDmods: {current.redmods.join(", ")}. Order them in Load order.</p> : null}
            <div className="actions">
              <Key icon="refresh" onClick={() => run("redeploy", async () => { await api.redeploy(current.key); toast(`Redeployed ${current.name}.`); reload(); })}>
                Redeploy
              </Key>
              {current.modId ? (
                <Key icon="link" variant="ghost" onClick={() => api.openNexus(current.modId!)}>
                  Nexus page
                </Key>
              ) : null}
              <Key icon="trash" onClick={() => run("remove", async () => { await api.removeMod(current.key); toast(`Removed ${current.name}. Its archive stays in downloads.`); setSel(null); reload(); refresh(); })}>
                Remove
              </Key>
            </div>
          </div>
        )}
      </Module>
    </div>
  );
}
