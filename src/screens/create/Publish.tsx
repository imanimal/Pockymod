// Publish a new version of a file that is already on Nexus, through the Upload API. The file is picked from the
// author's own mods (and, with a key, from the list Nexus keeps for uploads), so nobody has to look up an ID.
import { useEffect, useState } from "react";
import { Empty, Key, Lcd, Module, Pill, Seg, Switch, bytesText } from "../../ui/pocket";
import { authoring, creator, errorText, type MyMods, type UploadTarget } from "../../lib/api";
import { shortDate } from "../../lib/text";
import { toast } from "../../ui/toast";
import { bump } from "../../lib/version";
import { prefillFor } from "./MyMods";
import { fileCategoryText, fileName, type FileCategory, type Pack, type PublishPrefill } from "./shared";

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
  const [busy, setBusy] = useState(false);
  const [prog, setProg] = useState<[number, number] | null>(null);
  const [targets, setTargets] = useState<UploadTarget[] | null>(null);
  const [targetsErr, setTargetsErr] = useState<string | null>(null);
  useEffect(() => { let off: (() => void) | undefined; creator.onProgress(setProg).then((u) => { off = u; }); return () => off?.(); }, []);

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
  const next = current ? bump(current) : null;
  const zip = pack.zip;
  const ready = !!zip && !!fileId && !!pack.version && hasKey && !busy;

  const publish = async () => {
    setBusy(true); setProg(null);
    try {
      const id = await creator.publish({ file: zip!, fileId: /^\d+$/.test(fileId) ? Number(fileId) : fileId, modId: modId ? Number(modId) : null, version: pack.version,
        displayName: display || pack.name || null, category, changelog: changelog || null, archiveExisting: archiveOld, updateModVersion: bumpVersion });
      toast(`Published version ${pack.version}${id ? ` (version ${id})` : ""}. Nexus scans it before players see it.`);
      onPublished();
    } catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
  };

  return (
    <div className="grid2 cr-publish" style={{ alignItems: "start" }}>
      <Module icon="share" title="Publish a new version" band="hot" count="Upload API, beta">
        <p>Adds a version to a file you already have on Nexus. The page and its first file are made on the website once; every update after that can go from here.</p>
        {!hasKey && <div className="note warn" style={{ marginTop: ".6em" }}>Publishing needs your Nexus API key. Add it in Settings; it stays in Windows Credential Manager.</div>}

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
          {mod && (mod.files.length === 0 ? <p className="dim">This mod has no current file. Add its first file on the website.</p> : (
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
            <div className="sub-h">Files Nexus takes uploads for</div>
            <div className="rows">
              {targets.map((t) => (
                <button key={t.id} type="button" className={`row${t.id === fileId ? " sel" : ""}`} onClick={() => { setFileId(t.id); if (t.name) setDisplay(t.name); }}>
                  <span className="m-main"><span className="m-t">{t.name || "Unnamed file"}</span>
                    <span className="m-meta">{t.versionsCount != null && <span>{t.versionsCount} versions</span>}{t.lastUploadedAt && <span>last {shortDate(t.lastUploadedAt)}</span>}{t.isActive === false && <span>no current version</span>}</span></span>
                  <span className="m-side mono">{t.id}</span>
                </button>
              ))}
            </div>
          </>}
          {targetsErr && <p className="dim hint">Nexus did not list the files for uploads ({targetsErr}). The file's update chain id is used.</p>}
          <p className="dim hint">File ID for the upload: <span className="mono hi">{fileId || "none"}</span>{fileId && (listed ? " (from Nexus's list)" : " (the file's update chain; if Nexus refuses it, compare it with the Files tab, \"Advanced\")")}.</p>
          <Key size="sm" variant="ghost" onClick={() => setByHand(true)}>Enter the IDs by hand</Key>
        </> : <>
          <div className="fgrid">
            <span className="lab">File ID</span><label className="fbox" style={{ flex: "none" }}><input className="finput" value={fileId} onChange={(e) => setFileId(e.target.value.trim())} aria-label="File ID" /></label>
            <span className="lab">Mod ID</span><label className="fbox" style={{ flex: "none" }}><input className="finput" inputMode="numeric" value={modId} onChange={(e) => setModId(e.target.value.replace(/\D/g, ""))} placeholder="for the changelog" aria-label="Mod ID" /></label>
          </div>
          <p className="dim hint">The file ID is on the mod page: Files tab, "Advanced", or the file's edit menu on Manage files. {own ? "" : "Show your mods in My mods to pick it instead."}</p>
          {own && <Key size="sm" variant="ghost" onClick={() => setByHand(false)}>Pick from my mods</Key>}
        </>}
      </Module>

      <Module icon="file" title="The new version" count={zip ? fileName(zip) : "no archive yet"}>
        {!zip ? <Empty icon="file" title="Pack your folder or choose an archive" actions={<>
          <Key icon="install" onClick={onPack}>Check and pack a folder</Key>
          <Key variant="ghost" icon="file" onClick={() => void pack.pick(false)}>Choose an archive</Key>
        </>} /> : <div className="actions"><Pill tone="ok">{fileName(zip)}</Pill><Key size="sm" variant="ghost" onClick={() => void pack.pick(false)}>Another archive</Key></div>}
        <div className="fgrid" style={{ marginTop: ".7em" }}>
          <span className="lab">Version</span><label className="fbox" style={{ flex: "none" }}><input className="finput" value={pack.version} onChange={(e) => pack.set({ version: e.target.value })} aria-label="Version" /></label>
          <span className="lab">Name</span><label className="fbox" style={{ flex: "none" }}><input className="finput" value={display} onChange={(e) => setDisplay(e.target.value)} placeholder={pack.name || "The file's name on Nexus"} aria-label="File name on Nexus" /></label>
          <span className="lab">Category</span><Seg label="Category" value={category} onChange={setCategory} options={[{ value: "main", label: "Main" }, { value: "optional", label: "Optional" }, { value: "miscellaneous", label: "Misc" }]} />
          <span className="lab">Changelog</span><textarea rows={4} value={changelog} onChange={(e) => setChangelog(e.target.value)} aria-label="Changelog" placeholder="What changed in this version" />
        </div>
        {current && <div className="actions" style={{ marginTop: ".5em" }}><span className="dim" style={{ fontSize: ".86em" }}>The file is at version {current} now.</span>
          {next && next !== pack.version && <Key size="sm" variant="ghost" onClick={() => pack.set({ version: next })}>Use {next}</Key>}</div>}
        {changelog.trim() && !modId && <p className="dim hint">The changelog needs the mod: pick it, or enter its ID.</p>}
        <div className="stack" style={{ gap: ".4em", marginTop: ".6em" }}>
          <Switch checked={archiveOld} onChange={setArchiveOld}>Move the previous version to Old files</Switch>
          <Switch checked={bumpVersion} onChange={setBumpVersion}>Set the mod's version to {pack.version || "this version"}</Switch>
        </div>
        {prog && <Lcd size="sm" label="Uploaded" value={bytesText(prog[0]).split(" ")[0]} unit={`${bytesText(prog[0]).split(" ")[1]} / ${bytesText(prog[1])}`} bar={prog[1] ? prog[0] / prog[1] : 0} />}
        <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".6em" }}>
          <Key variant="dark" icon="share" disabled={!ready} onClick={() => void publish()}>{busy ? "Publishing" : "Publish"}</Key>
        </div>
      </Module>
    </div>
  );
}

