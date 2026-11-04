// For mod authors: check a mod the way players' managers will see it, pack it cleanly, and publish a new version
// without leaving the app. A brand-new mod page still starts on the Nexus upload form, opened here.
import { useEffect, useState } from "react";
import { Empty, Key, Lcd, Module, Pill, Seg, Switch, bytesText } from "../ui/pocket";
import { creator, errorText, nexusPages, pickArchive, pickFolder, type CreatorCheck } from "../lib/api";
import { toast } from "../ui/toast";

export function Create() {
  const [path, setPath] = useState<string | null>(null);
  const [isFolder, setIsFolder] = useState(false);
  const [name, setName] = useState("");
  const [version, setVersion] = useState("1.0.0");
  const [check, setCheck] = useState<CreatorCheck | null>(null);
  const [zip, setZip] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const pick = async (folder: boolean) => {
    const p = folder ? await pickFolder("Your mod's folder") : await pickArchive();
    if (!p) return;
    const n = p.split(/[\\/]/).pop()?.replace(/\.(zip|7z|rar)$/i, "") ?? "My mod";
    setPath(p); setIsFolder(folder); setZip(folder ? null : p); if (!name) setName(n);
    setBusy(true);
    try { setCheck(await creator.check(p, name || n)); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
  };

  return (
    <div className="grid2" style={{ alignItems: "start", maxWidth: "84em" }}>
      <div className="stack">
        <Module icon="file" title="1 · Check your mod" count={check ? `${check.files} files` : undefined}>
          <p>Pick your mod's folder (or a packed archive). Pocket Mods shows where every file would land for a player, and what breaks installs.</p>
          <div className="actions" style={{ marginTop: ".6em" }}>
            <Key icon="folder" onClick={() => pick(true)} disabled={busy}>Choose a folder</Key>
            <Key variant="ghost" icon="file" onClick={() => pick(false)} disabled={busy}>Choose an archive</Key>
          </div>
          {path && <p className="dim mono" style={{ fontSize: ".78em", marginTop: ".5em", overflowWrap: "anywhere" }}>{path}</p>}
          {check && <>
            <div className="lcds" style={{ marginTop: ".6em" }}>
              <Lcd size="sm" label="Installs" value={String(check.plan.files.length).padStart(2, "0")} sub={check.plan.modType ?? "type unknown"} />
              <Lcd size="sm" label="Problems" value={String(check.problems.length).padStart(2, "0")} tone={check.problems.length ? "bad" : "ok"} />
            </div>
            {check.problems.map((p) => <div key={p} className="note bad" style={{ marginTop: ".4em" }}>{p}</div>)}
            {check.tips.map((p) => <div key={p} className="note warn" style={{ marginTop: ".4em" }}>{p}</div>)}
            {check.problems.length === 0 && <div className="note" style={{ marginTop: ".4em" }}>Ready: every file has a place in the game.</div>}
            <div className="sub-h">Where the files go</div>
            <div className="log" style={{ maxHeight: "12em" }}>{check.plan.files.slice(0, 200).map((f) => f.target).join("\n") || "nothing"}</div>
          </>}
        </Module>

        <Module icon="install" title="2 · Pack it">
          <div className="fgrid">
            <span className="lab">Name</span><label className="fbox" style={{ flex: "none" }}><input className="finput" value={name} onChange={(e) => setName(e.target.value)} aria-label="Mod name" /></label>
            <span className="lab">Version</span><label className="fbox" style={{ flex: "none" }}><input className="finput" value={version} onChange={(e) => setVersion(e.target.value)} aria-label="Version" /></label>
          </div>
          <p className="dim" style={{ fontSize: ".86em", marginTop: ".5em" }}>A .zip with each file at its game path (archive/pc/mod, r6/scripts …), docs and tools left out, saved next to your folder.</p>
          <div className="actions" style={{ marginTop: ".5em" }}>
            <Key icon="install" disabled={!path || !isFolder || busy || !check?.plan.files.length} onClick={async () => {
              setBusy(true);
              try { const z = await creator.pack(path!, name, version); setZip(z); toast("Packed."); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
            }}>Pack a clean zip</Key>
            {zip && <Pill tone="ok">{zip.split(/[\\/]/).pop()}</Pill>}
          </div>
        </Module>
      </div>

      <Publish zip={zip} version={version} name={name} />
    </div>
  );
}

function Publish({ zip, version, name }: { zip: string | null; version: string; name: string }) {
  const [fileId, setFileId] = useState("");
  const [modId, setModId] = useState("");
  const [category, setCategory] = useState<"main" | "optional" | "miscellaneous">("main");
  const [changelog, setChangelog] = useState("");
  const [archiveOld, setArchiveOld] = useState(true);
  const [bumpVersion, setBumpVersion] = useState(true);
  const [busy, setBusy] = useState(false);
  const [prog, setProg] = useState<[number, number] | null>(null);
  useEffect(() => { let off: (() => void) | undefined; creator.onProgress(setProg).then((u) => { off = u; }); return () => off?.(); }, []);
  return (
    <Module icon="share" title="3 · Publish" band="hot" count="Upload API, beta">
      <p>Add a new version to a file you already have on Nexus. Its file ID is on your mod page: Files tab, "Advanced". Uses your API key from Settings.</p>
      <div className="fgrid" style={{ marginTop: ".6em" }}>
        <span className="lab">File ID</span><label className="fbox" style={{ flex: "none" }}><input className="finput" inputMode="numeric" value={fileId} onChange={(e) => setFileId(e.target.value.replace(/\D/g, ""))} aria-label="File ID" /></label>
        <span className="lab">Mod ID</span><label className="fbox" style={{ flex: "none" }}><input className="finput" inputMode="numeric" value={modId} onChange={(e) => setModId(e.target.value.replace(/\D/g, ""))} placeholder="for the changelog" aria-label="Mod ID" /></label>
        <span className="lab">Category</span><Seg label="Category" value={category} onChange={setCategory} options={[{ value: "main", label: "Main" }, { value: "optional", label: "Optional" }, { value: "miscellaneous", label: "Misc" }]} />
        <span className="lab">Changelog</span><textarea rows={4} value={changelog} onChange={(e) => setChangelog(e.target.value)} aria-label="Changelog" placeholder="What changed in this version" />
      </div>
      <div className="stack" style={{ gap: ".4em", marginTop: ".6em" }}>
        <Switch checked={archiveOld} onChange={setArchiveOld}>Move the previous version to Old files</Switch>
        <Switch checked={bumpVersion} onChange={setBumpVersion}>Set the mod's version to {version || "this version"}</Switch>
      </div>
      {prog && <Lcd size="sm" label="Uploaded" value={bytesText(prog[0]).split(" ")[0]} unit={`${bytesText(prog[0]).split(" ")[1]} / ${bytesText(prog[1])}`} bar={prog[1] ? prog[0] / prog[1] : 0} />}
      {!zip && <Empty icon="file" title="Pack or choose an archive first" />}
      <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".6em" }}>
        <Key variant="ghost" icon="web" onClick={() => nexusPages.url("https://www.nexusmods.com/cyberpunk2077/mods/add").catch((e) => toast(errorText(e), true))}>New mod page on Nexus</Key>
        <Key variant="dark" icon="share" disabled={!zip || !fileId || !version || busy} onClick={async () => {
          setBusy(true); setProg(null);
          try {
            const id = await creator.publish({ file: zip!, fileId: Number(fileId), modId: modId ? Number(modId) : null, version, displayName: name || null, category,
              changelog: changelog || null, archiveExisting: archiveOld, updateModVersion: bumpVersion });
            toast(`Published version ${version}${id ? ` (version ${id})` : ""}. Nexus scans it before players see it.`);
          } catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
        }}>{busy ? "Publishing" : "Publish"}</Key>
      </div>
    </Module>
  );
}
