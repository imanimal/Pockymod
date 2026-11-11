// Publish a new version of a file that is already on Nexus, through the Upload API. The file is picked from the
// author's own mods (and, with a key, from the list Nexus keeps for uploads), so nobody has to look up an ID.
import { useEffect, useState } from "react";
import { Empty, Key, Lcd, Module, Pill, Seg, Switch, bytesText, useBusy } from "../../ui/pocket";
import { authoring, creator, errorText, type MyMods, type UploadTarget } from "../../lib/api";
import { plural, shortDate } from "../../lib/text";
import { toast } from "../../ui/toast";
import { SettingsLink } from "../../ui/SettingsLink";
import { bump } from "../../lib/version";
import { prefillFor } from "./MyMods";
import { fileCategoryText, fileName, openNexus, type FileCategory, type Pack, type PublishPrefill } from "./shared";

export function Publish({ pack, mine, hasKey, prefill, onPack, onPublished }: {
  pack: Pack; mine: MyMods | null; hasKey: boolean; prefill: PublishPrefill | null; onPack: () => void; onPublished: () => void;
}) {
  const own = mine?.own ? mine : null;
  const [modId, setModId] = useState("");
  const [fileId, setFileId] = useState("");
  const [display, setDisplay] = useState("");
  const [current, setCurrent] = useState<string | null>(null);
  const [byHand, setByHand] = useState(!own);
  const [category, setCategory] = useState<FileCategory>("main");
  const [changelog, setChangelog] = useState("");
  const [archiveOld, setArchiveOld] = useState(true);
  const [bumpVersion, setBumpVersion] = useState(true);
  const [busy, run] = useBusy();
  const [prog, setProg] = useState<[number, number] | null>(null);
  const [targets, setTargets] = useState<UploadTarget[] | null>(null);
  const [targetsErr, setTargetsErr] = useState<string | null>(null);
  useEffect(() => {
    // the listener can arrive after the tab has closed: drop it then instead of leaking it
    let off: (() => void) | undefined, gone = false;
    creator.onProgress(setProg).then((u) => { if (gone) u(); else off = u; }).catch(() => {});
    return () => { gone = true; off?.(); };
  }, []);

  const choose = (p: PublishPrefill) => {
    setModId(String(p.modId)); setFileId(p.fileId); setDisplay(p.fileName); setCategory(p.category); setCurrent(p.currentVersion); setByHand(false);
  };
  useEffect(() => { if (prefill) choose(prefill); }, [prefill]);
  useEffect(() => {
    // nothing picked yet: start on the most recently updated mod's main file
    if (prefill || modId || !own?.mods.length) return;
    const m = own.mods[0];
    const f = m.files.find((x) => x.category === "MAIN") ?? m.files[0];
    if (f) choose(prefillFor(m, f));
  }, [own, prefill, modId]);

  // with a key, ask Nexus which files of the mod take uploads, and use its id for the picked one
  useEffect(() => {
    setTargets(null); setTargetsErr(null);
    if (!hasKey || !modId || byHand) return;
    let live = true;
    authoring.uploadTargets(Number(modId)).then((t) => { if (live) setTargets(t); }).catch((e) => { if (live) setTargetsErr(errorText(e)); });
    return () => { live = false; };
  }, [hasKey, modId, byHand]);
  useEffect(() => {
    const hit = targets?.find((t) => t.groupId != null && String(t.groupId) === fileId);
    if (hit && hit.id !== fileId) setFileId(hit.id);
  }, [targets, fileId]);

  const mod = own?.mods.find((m) => String(m.modId) === modId) ?? null;
  const listed = !!targets?.some((t) => t.id === fileId);
  // asking Nexus for the upload targets: hold the "not listed" warning until it answers
  const asking = hasKey && !!modId && !byHand && targets === null && !targetsErr;
  const next = current ? bump(current) : null;
  const zip = pack.zip;
  const why = !hasKey ? "Needs your Nexus API key" : !zip ? "Pack your folder or choose an archive first" : !fileId ? "Pick the file this is a new version of"
    : !pack.version ? "Set the version" : pack.busy === "pack" ? "Wait for the zip to finish packing" : null;

  const publish = () => run(async () => {
    if (!zip) return;
    setProg(null);
    await creator.publish({ file: zip, fileId: /^\d+$/.test(fileId) ? Number(fileId) : fileId, modId: modId ? Number(modId) : null, version: pack.version,
      displayName: display || pack.name || null, category, changelog: changelog || null, archiveExisting: archiveOld, updateModVersion: bumpVersion });
    toast(`Published ${pack.version}. Nexus scans it first`);
    onPublished();
  });

  return (
    <div className="grid2 cr-publish" style={{ alignItems: "start" }}>
      <Module icon="share" title="Publish a new version" band="hot" count="Upload API, beta">
        {!hasKey && <div className="note warn">Needs your Nexus API key, add it in <SettingsLink section="Nexus account" /></div>}

        <div className="sub-h">Which file</div>
        {own && !byHand ? <>
          <label className="cr-field"><span className="lab">Mod</span>
            <select value={modId} onChange={(e) => {
              const m = own.mods.find((x) => String(x.modId) === e.target.value);
              const f = m?.files.find((x) => x.category === "MAIN") ?? m?.files[0];
              if (m && f) choose(prefillFor(m, f)); else { setModId(e.target.value); setFileId(""); }
            }} aria-label="Mod">
              {own.mods.map((m) => <option key={m.modId} value={m.modId}>{m.name} (v{m.version})</option>)}
            </select>
          </label>
          {mod && (mod.files.length === 0 ? <p className="dim">No current file. Add the first one on Nexus</p> : (
            <div className="rows" role="radiogroup" aria-label="File">
              {mod.files.map((f) => {
                const on = (f.groupId != null && String(f.groupId) === fileId) || targets?.find((t) => t.id === fileId)?.groupId === f.groupId;
                return (
                  <button key={f.fileId} type="button" role="radio" aria-checked={on} className={`row${on ? " sel" : ""}`} onClick={() => choose(prefillFor(mod, f))}>
                    <span className="m-main"><span className="m-t">{f.name}</span><span className="m-meta"><span>{fileCategoryText(f.category)}</span><span>now v{f.version}</span><span>{bytesText(f.sizeInBytes ?? f.size * 1024)}</span></span></span>
                  </button>
                );
              })}
            </div>
          ))}
          {targets && targets.length > 0 && <>
            <div className="sub-h">Upload targets</div>
            <div className="rows">
              {targets.map((t) => (
                <button key={t.id} type="button" className={`row${t.id === fileId ? " sel" : ""}`} onClick={() => { setFileId(t.id); if (t.name) setDisplay(t.name); }}>
                  <span className="m-main"><span className="m-t">{t.name || "Unnamed file"}</span>
                    <span className="m-meta">{t.versionsCount != null && <span>{plural(t.versionsCount, "version")}</span>}{t.lastUploadedAt && <span>last {shortDate(t.lastUploadedAt)}</span>}{t.isActive === false && <span>no current version</span>}</span></span>
                  <span className="m-side mono">{t.id}</span>
                </button>
              ))}
            </div>
          </>}
          {targetsErr && <p className="dim hint">Nexus didn't list upload targets ({targetsErr}). Using the file's update chain ID</p>}
          <p className="dim hint">File ID <span className="mono hi">{fileId || "none"}</span>{fileId && !asking && (listed ? "" : " (update chain, check Files tab, Advanced if refused)")}</p>
          <Key size="sm" variant="ghost" onClick={() => setByHand(true)}>Enter IDs manually</Key>
        </> : <>
          <div className="fgrid">
            <span className="lab">File ID</span><label className="fbox" style={{ flex: "none" }}><input className="finput" value={fileId} onChange={(e) => setFileId(e.target.value.trim())} aria-label="File ID" /></label>
            <span className="lab">Mod ID</span><label className="fbox" style={{ flex: "none" }}><input className="finput" inputMode="numeric" value={modId} onChange={(e) => setModId(e.target.value.replace(/\D/g, ""))} placeholder="for the changelog" aria-label="Mod ID" /></label>
          </div>
          <p className="dim hint">File ID is on the mod page under Files tab, Advanced</p>
          {own && <Key size="sm" variant="ghost" onClick={() => setByHand(false)}>Pick from my mods</Key>}
        </>}
      </Module>

      <Module icon="file" title="The new version" count={zip ? fileName(zip) : "no archive yet"}>
        {!zip ? <Empty icon="file" title="Pack your folder or choose an archive" actions={<>
          <Key icon="install" onClick={onPack}>Check and pack a folder</Key>
          <Key variant="ghost" icon="file" busy={pack.busy === "archive"} disabled={!!pack.busy || busy} onClick={() => void pack.pick(false)}>Choose an archive</Key>
        </>} /> : <div className="actions"><Pill tone="ok">{fileName(zip)}</Pill>
          <Key size="sm" variant="ghost" busy={pack.busy === "archive"} disabled={!!pack.busy || busy} onClick={() => void pack.pick(false)}>Another archive</Key></div>}
        <div className="fgrid" style={{ marginTop: ".7em" }}>
          <span className="lab">Version</span><label className="fbox" style={{ flex: "none" }}><input className="finput" value={pack.version} onChange={(e) => pack.set({ version: e.target.value })} aria-label="Version" /></label>
          <span className="lab">Name</span><label className="fbox" style={{ flex: "none" }}><input className="finput" value={display} onChange={(e) => setDisplay(e.target.value)} placeholder={pack.name || "File name on Nexus"} aria-label="File name on Nexus" /></label>
          <span className="lab">Category</span><Seg label="Category" value={category} onChange={setCategory} options={[{ value: "main", label: "Main" }, { value: "optional", label: "Optional" }, { value: "miscellaneous", label: "Misc" }]} />
          <span className="lab">Changelog</span><textarea rows={4} value={changelog} onChange={(e) => setChangelog(e.target.value)} aria-label="Changelog" placeholder="What changed in this version" />
        </div>
        {current && <div className="actions" style={{ marginTop: ".5em" }}><span className="dim" style={{ fontSize: ".86em" }}>Currently {current}</span>
          {next && next !== pack.version && <Key size="sm" variant="ghost" onClick={() => pack.set({ version: next })}>Use {next}</Key>}</div>}
        {changelog.trim() && !modId && <p className="dim hint">A changelog needs the mod ID</p>}
        <div className="stack" style={{ gap: ".4em", marginTop: ".6em" }}>
          <Switch checked={archiveOld} onChange={setArchiveOld}>Move the previous version to Old files</Switch>
          <Switch checked={bumpVersion} onChange={setBumpVersion}>Set the mod's version to {pack.version || "this version"}</Switch>
        </div>
        {prog && <Lcd size="sm" label="Uploaded" value={bytesText(prog[0]).split(" ")[0]} unit={`${bytesText(prog[0]).split(" ")[1]} / ${bytesText(prog[1])}`} bar={prog[1] ? prog[0] / prog[1] : 0} />}
        <p className="dim hint" style={{ marginTop: ".6em" }}>If this version adds parts made with AI, update the AI statement on the mod's Nexus page{mod && <> (<button type="button" className="linkish" onClick={() => openNexus(mod.links.edit)} title="The mod's edit page on Nexus">edit the page</button>)</>}. Nexus asks for it whenever an update adds AI-made parts.</p>
        <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".6em" }}>
          <Key variant="dark" icon="share" busy={busy ? (prog && prog[1] ? prog[0] / prog[1] : true) : undefined} disabled={!!why} title={why ?? undefined}
            onClick={() => void publish()}>Publish</Key>
        </div>
      </Module>
    </div>
  );
}

