// Settings: where the game and the mod library are, the copy fallback, and the nxm:// handler.
import { useEffect, useState } from "react";
import { Key, Module, Pill, Switch, bytesText, useProgress } from "../ui/pocket";
import { browse, downloadsCleanup, downloadsUsage, errorText, game, handler, paths, pickFolder, settings, testHandler, type GameInstall, type HandlerStatus, type PathsView } from "../lib/api";
import { toast } from "../ui/toast";
import { relocate } from "../lib/api";
import { MoveLibraryDialog, relinkText } from "./MoveLibrary";

export function GameFolders() {
  const [p, setP] = useState<PathsView | null>(null);
  const [h, setH] = useState<HandlerStatus | null>(null);
  const [found, setFound] = useState<GameInstall[] | null>(null);
  const [patches, setPatches] = useState<string[]>([]);
  const [patch, setPatch] = useState<string>("");
  const [detected, setDetected] = useState<string | null>(null);
  const [usage, setUsage] = useState<{ files: number; bytes: number; installedFiles: number; installedBytes: number } | null>(null);
  const [keep, setKeep] = useState(true);
  // the key whose work is running
  const [working, setWorking] = useState<string | null>(null);
  // setting the game folder can relink a moved copy's mod files, which reports how far it is
  const relinkP = useProgress(working === "choose" || working === "relink" || !!working?.startsWith("use"), relocate.onRelink);
  const run = async (key: string, job: () => Promise<unknown>) => {
    setWorking(key);
    try { await job(); } catch (e) { toast(errorText(e), true); } finally { setWorking(null); }
  };
  useEffect(() => { downloadsUsage().then(setUsage).catch(() => {}); settings.get("downloads.keep").then((v) => setKeep(v !== "0")).catch(() => {}); }, []);
  useEffect(() => {
    paths.get().then(setP).catch((e) => toast(errorText(e), true));
    handler.status().then(setH).catch(() => {});
    browse.patches().then((l) => setPatches(l.map(([x]) => x.version))).catch(() => {});
    settings.get("game.patch").then((v) => setPatch(v ?? "")).catch(() => {});
  }, []);
  useEffect(() => { if (p?.gameDir) game.inspect().then((g) => setDetected(g?.detectedPatch ?? null)).catch(() => {}); }, [p?.gameDir]);
  const save = (x: Parameters<typeof paths.set>[0]) => paths.set(x).then(setP).catch((e) => toast(errorText(e), true));
  // a new game folder: when it is a moved copy of the game, its mod files are linked to the library again
  const setGame = async (dir: string) => { const { paths: v, relink } = await relocate.setGame(dir); setP(v); if (relink) toast(relinkText(relink)); };
  const [moving, setMoving] = useState(false);
  const busy = working !== null;
  const path = (text: string) => p ? <code style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{text}</code>
    : <span className="skel skel-line" style={{ flex: 1 }} aria-hidden="true" />;

  return (
    <Module icon="folder" title="Game and mod folders" count={p ? (p.gameLooksRight ? "game found" : "not set") : undefined}>
      <div className="fgrid">
        <span className="lab">Cyberpunk 2077</span>
        <div className="actions" style={{ flexWrap: "nowrap" }}>
          {path(p?.gameDir ?? "Not set")}
          <Key size="sm" icon="browse" disabled={busy} busy={working === "find"} onClick={() => run("find", async () => setFound(await game.detect()))} title="Look in Steam, GOG and Epic">Find</Key>
          <Key size="sm" icon="folder" disabled={busy} busy={working === "choose" && (relinkP ?? true)} onClick={() => run("choose", async () => { const d = await pickFolder("Cyberpunk 2077 folder (holds bin\\x64)"); if (d) await setGame(d); })}>Choose</Key>
        </div>
        {found && <>
          <span className="lab">Found</span>
          <div className="stack" style={{ gap: ".3em" }}>
            {found.length === 0 && <span className="dim">No install found. Choose the folder</span>}
            {found.map((g) => (
              <div key={g.path} className="actions" style={{ flexWrap: "nowrap" }}>
                <Pill tone={g.exists ? "ok" : undefined}>{g.store}</Pill>
                <code style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{g.path}</code>
                <Key size="sm" disabled={!g.exists || busy} busy={working === `use${g.path}` && (relinkP ?? true)} title={g.exists ? undefined : "That folder is gone"}
                  onClick={() => run(`use${g.path}`, async () => { await setGame(g.path); setFound(null); })}>Use</Key>
              </div>
            ))}
            {found.some((g) => g.store === "Epic") && <span className="dim" style={{ fontSize: ".84em" }}>Epic support is best effort</span>}
          </div>
        </>}
        <span className="lab">Game patch</span>
        <div className="actions">
          {detected ? <Pill tone="ok">Detected {detected}</Pill> : <span className="dim" style={{ fontSize: ".86em" }}>Not detected</span>}
          <select value={patch} onChange={(e) => { setPatch(e.target.value); settings.set("game.patch", e.target.value).catch((err) => toast(errorText(err), true)); }} aria-label="Game patch">
            <option value="">{detected ? "Detected" : "Pick patch"}</option>
            {patches.map((x) => <option key={x} value={x}>{x}</option>)}
          </select>
        </div>
        <span className="lab">Mod library</span>
        <div className="actions" style={{ flexWrap: "nowrap" }}>
          {path(p?.libraryDir ?? "Set the game folder first")}
          <Key size="sm" icon="folder" disabled={!p?.libraryDir} onClick={() => setMoving(true)} title={p?.libraryDir ? "Move the library and its contents" : "Set the game folder first"}>Move</Key>
          <Key size="sm" variant="ghost" onClick={async () => { const d = await pickFolder("Library folder, on the game's drive"); if (d) save({ libraryDir: d }); }} title="Use another folder without moving anything">Choose</Key>
        </div>
        <span className="lab">Deploy</span>
        <div className="actions">
          {p?.sameVolume === true && <Pill tone="ok">Same drive: hard links</Pill>}
          {p?.sameVolume === false && <Pill tone="over">Different drives</Pill>}
          <Switch checked={!!p?.allowCopy} onChange={(v) => save({ allowCopy: v })}>Copy when hard links are impossible</Switch>
          {p?.gameLooksRight && <Key size="sm" variant="ghost" icon="link" disabled={busy} busy={working === "relink" && (relinkP ?? true)} onClick={() => run("relink", async () => toast(relinkText(await relocate.relinkGame())))}
            title="Relink mod files after moving the game">Check links</Key>}
        </div>
      </div>
      {p?.gameDir && !p.gameLooksRight && <div className="note bad" style={{ marginTop: ".7em" }}>No bin\x64 in that folder. Pick the one with bin, archive and r6</div>}
      {p?.inProgramFiles && <div className="note warn" style={{ marginTop: ".7em" }}>Game is under Program Files, where Windows can block writes. If installs fail, move it in Steam (Properties, Installed files, Move install folder)</div>}
      {moving && <MoveLibraryDialog onClose={() => setMoving(false)} onMoved={() => paths.get().then(setP).catch(() => {})} />}
      {p?.sameVolume === false && <div className="note warn" style={{ marginTop: ".7em" }}>Library is on another drive, so files get copied. Use one on the game's drive{p.defaultLibrary ? `, like ${p.defaultLibrary}` : ""}</div>}

      <div className="sub-h">Downloaded archives</div>
      <div className="actions">
        {usage && <span className="dim" style={{ fontSize: ".88em" }}>{usage.files} archives, {bytesText(usage.bytes)} · {usage.installedFiles} installed ({bytesText(usage.installedBytes)})</span>}
        {usage && usage.installedFiles > 0 && <Key size="sm" icon="trash" disabled={busy} busy={working === "cleanup"} title="Their mods stay installed"
          onClick={() => run("cleanup", async () => { const b = await downloadsCleanup(); toast(`Freed ${bytesText(b)}`); setUsage(await downloadsUsage()); })}>Delete installed archives</Key>}
      </div>
      <Switch checked={keep} onChange={(v) => { setKeep(v); settings.set("downloads.keep", v ? "1" : "0").catch((e) => { setKeep(!v); toast(errorText(e), true); }); }}><span title="Reinstall without downloading again">Keep archives after installing</span></Switch>

      <div className="sub-h">nxm:// links</div>
      {h && !h.supported && <p className="dim">Windows only</p>}
      {h?.supported && (
        <div className="actions">
          {h.ours ? <Pill tone="ok">Pockymod opens nxm:// links</Pill> : <Pill tone="soon">{h.owner ? `${h.owner} opens nxm:// links` : "No app opens nxm:// links"}</Pill>}
          {!h.ours && <Key size="sm" icon="link" disabled={busy} busy={working === "register"}
            onClick={() => run("register", async () => { setH(await handler.register()); void settings.set("links.other", "0").catch(() => {}); toast("Pockymod opens nxm:// links"); })}>Use Pockymod</Key>}
          <Key size="sm" variant="ghost" disabled={busy} busy={working === "test"} onClick={() => run("test", async () => { await testHandler(); toast("Test link sent"); })} title="Send a test nxm:// link">Test</Key>
        </div>
      )}
    </Module>
  );
}
