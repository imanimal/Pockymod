// Library: what is installed (and deployed), downloads on disk that are not installed, and installing from a file.
import { useCallback, useEffect, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Empty, Key, Module, Pill, Tag, bytesText } from "../ui/pocket";
import { FRAMEWORKS, errorText, library, pickArchive, queueMod, type Conflict, type DeployOutcome, type DownloadRow, type InstallOutcome, type InstallPlan, type InstallRow } from "../lib/api";
import { toast } from "../ui/toast";

type Pending =
  | { kind: "conflicts"; installId: number; name: string; conflicts: Conflict[] }
  | { kind: "copy"; installId: number; name: string; reason: string }
  | { kind: "review"; download: { modId: number; fileId: number; name: string; version: string; archive: string }; plan: InstallPlan };

export function Library({ updates }: { updates?: Map<number, string> }) {
  const [rows, setRows] = useState<InstallRow[] | null>(null);
  const [downloads, setDownloads] = useState<DownloadRow[]>([]);
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const load = useCallback(() => {
    library.list().then(setRows).catch((e) => setErr(errorText(e)));
    library.downloads().then(setDownloads).catch(() => {});
  }, []);
  useEffect(() => {
    load();
    let off: (() => void) | undefined;
    library.onChanged(load).then((u) => { off = u; });
    return () => off?.();
  }, [load]);

  const handleOutcome = (o: InstallOutcome, name: string, download?: Extract<Pending, { kind: "review" }>["download"]) => {
    if (o.status === "installed") {
      toast(`Installed ${name}${o.deploy.outcome === "deployed" && o.deploy.copied ? ` (${o.deploy.copied} files copied)` : ""}.`);
      if (o.needsRedmodDeploy) toast("This is a REDmod: run the REDmod deploy from Load order.");
    } else if (o.status === "needsReview" && download) setPending({ kind: "review", download, plan: o.plan });
    else if (o.status === "staged") onDeployOutcome(o.installId, name, o.deploy);
    load();
  };
  const onDeployOutcome = (installId: number, name: string, d: DeployOutcome) => {
    if (d.outcome === "conflicts") setPending({ kind: "conflicts", installId, name, conflicts: d.conflicts });
    else if (d.outcome === "needsCopy") setPending({ kind: "copy", installId, name, reason: d.reason });
    else { toast(`Deployed ${name}.`); setPending(null); }
    load();
  };
  const run = async (f: () => Promise<void>) => { setBusy(true); try { await f(); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); } };

  const deploy = (r: { id: number; name: string }, takeConflicts = false, allowCopy = false) =>
    run(async () => onDeployOutcome(r.id, r.name, await library.deploy(r.id, { takeConflicts, allowCopy })));
  const uninstall = (r: InstallRow) => run(async () => {
    const rep = await library.uninstall(r.id);
    toast(`Removed ${r.name}: ${rep.removed} files${rep.restored.length ? `, ${rep.restored.length} put back` : ""}${rep.leftAlone.length ? `, ${rep.leftAlone.length} changed by hand were left` : ""}.`);
    load();
  });
  const installDownload = (d: { modId: number; fileId: number; name: string; version: string; archive: string }, acceptUnknown = false) =>
    run(async () => handleOutcome(await library.installFile({ ...d, acceptUnknown }), d.name, d));
  const installFromFile = () => run(async () => {
    const archive = await pickArchive();
    if (!archive) return;
    const name = archive.split(/[\\/]/).pop()?.replace(/\.(zip|7z|rar)$/i, "") ?? "Mod";
    // a manual archive has no Nexus ids: it gets local ids from its name, so reinstalling the same file replaces it
    const local = 900_000_000 + (Array.from(name).reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7) % 90_000_000);
    const d = { modId: local, fileId: local, name, version: "local", archive };
    handleOutcome(await library.installFile(d), name, d);
  });

  const notInstalled = downloads.filter((d) => !d.installed);

  return (
    <div className="stack" style={{ maxWidth: "68em" }}>
      <Module icon="library" title="Installed" count={rows ? `${rows.length} mods` : undefined} bodyClass="list"
        tools={<Key size="sm" variant="primary" icon="file" onClick={installFromFile} disabled={busy}>Install from file</Key>}>
        {err && <div className="note bad">{err}</div>}
        {rows?.length === 0 && <Empty icon="library" title="No mods installed">Find mods in Browse, or install an archive you already have.</Empty>}
        <div className="rows">
          {rows?.map((r) => (
            <div key={r.id} className={`row${r.deployedFiles === 0 ? " off" : ""}`}>
              <span className="m-main">
                <span className="m-t">{r.name}</span>
                <span className="m-meta"><span>v{r.version}</span><span>{r.deployedFiles} files in the game</span>
                  {updates?.get(r.modId) && <span><Pill tone="soon">Update {updates.get(r.modId)}</Pill></span>}</span>
                <span className="m-tags">
                  <Tag kind={r.modType === "unknown" || r.modType === "mixed" ? undefined : r.modType} square>{r.modType === "redmod" ? "REDmod" : r.modType}</Tag>
                  {r.frameworks.map((f) => <Tag key={f} kind={f}>{FRAMEWORKS.find((x) => x.key === f)?.short ?? f}</Tag>)}
                </span>
              </span>
              <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".4em" }}>
                {updates?.get(r.modId) && r.modId < 900_000_000 && <Key size="sm" icon="downloads" onClick={() => run(async () => { await queueMod(r.modId); toast(`${r.name} update queued.`); })} disabled={busy}>Update</Key>}
                {r.deployedFiles === 0 ? <><Pill tone="soon">Not deployed</Pill><Key size="sm" onClick={() => deploy(r)} disabled={busy}>Deploy</Key></> : <Pill tone="ok">Deployed</Pill>}
                <Key size="sm" variant="ghost" icon="trash" onClick={() => uninstall(r)} disabled={busy} aria-label={`Uninstall ${r.name}`} title="Uninstall" />
              </span>
            </div>
          ))}
        </div>
      </Module>

      <Module icon="downloads" title="Downloaded, not installed" count={notInstalled.length || undefined} bodyClass="list">
        {notInstalled.length === 0 ? <p className="dim" style={{ padding: ".6em" }}>Every downloaded archive is installed.</p> : (
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
        )}
      </Module>

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
          <p className="dim" style={{ fontSize: ".86em" }}>{pending.plan.files.length} files have a place. Check the mod's install notes on its page if unsure.</p>
          <div className="actions" style={{ justifyContent: "flex-end" }}>
            <Key variant="ghost" onClick={() => setPending(null)}>Cancel</Key>
            <Key onClick={() => { const d = pending.download; setPending(null); installDownload(d, true); }} disabled={busy}>Install the rest</Key>
          </div>
        </Dialog>
      )}
    </div>
  );
}
