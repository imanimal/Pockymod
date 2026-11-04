// Settings: where the game and the mod library are, the copy fallback, and the nxm:// handler.
import { useEffect, useState } from "react";
import { Key, Module, Pill, Switch } from "../ui/pocket";
import { browse, errorText, game, handler, paths, pickFolder, settings, type GameInstall, type HandlerStatus, type PathsView } from "../lib/api";
import { toast } from "../ui/toast";

export function GameFolders() {
  const [p, setP] = useState<PathsView | null>(null);
  const [h, setH] = useState<HandlerStatus | null>(null);
  const [found, setFound] = useState<GameInstall[] | null>(null);
  const [patches, setPatches] = useState<string[]>([]);
  const [patch, setPatch] = useState<string>("");
  const [detected, setDetected] = useState<string | null>(null);
  useEffect(() => {
    paths.get().then(setP).catch((e) => toast(errorText(e), true));
    handler.status().then(setH).catch(() => {});
    browse.patches().then((l) => setPatches(l.map(([x]) => x.version))).catch(() => {});
    settings.get("game.patch").then((v) => setPatch(v ?? "")).catch(() => {});
  }, []);
  useEffect(() => { if (p?.gameDir) game.inspect().then((g) => setDetected(g?.detectedPatch ?? null)).catch(() => {}); }, [p?.gameDir]);
  const save = (x: Parameters<typeof paths.set>[0]) => paths.set(x).then(setP).catch((e) => toast(errorText(e), true));

  return (
    <Module icon="folder" title="Game and mod folders" count={p?.gameLooksRight ? "game found" : "not set"}>
      <div className="fgrid">
        <span className="lab">Cyberpunk 2077</span>
        <div className="actions" style={{ flexWrap: "nowrap" }}>
          <code style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p?.gameDir ?? "Not set"}</code>
          <Key size="sm" icon="browse" onClick={() => game.detect().then(setFound).catch((e) => toast(errorText(e), true))}>Find</Key>
          <Key size="sm" icon="folder" onClick={async () => { const d = await pickFolder("The Cyberpunk 2077 folder (it holds bin\\x64)"); if (d) save({ gameDir: d }); }}>Choose</Key>
        </div>
        {found && <>
          <span className="lab">Found</span>
          <div className="stack" style={{ gap: ".3em" }}>
            {found.length === 0 && <span className="dim">No Steam, GOG or Epic install found. Choose the folder by hand.</span>}
            {found.map((g) => (
              <div key={g.path} className="actions" style={{ flexWrap: "nowrap" }}>
                <Pill tone={g.exists ? "ok" : undefined}>{g.store}</Pill>
                <code style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{g.path}</code>
                <Key size="sm" disabled={!g.exists} onClick={() => { save({ gameDir: g.path }); setFound(null); }}>Use</Key>
              </div>
            ))}
            {found.some((g) => g.store === "Epic") && <span className="dim" style={{ fontSize: ".84em" }}>Epic installs are supported on a best-effort basis.</span>}
          </div>
        </>}
        <span className="lab">Game patch</span>
        <div className="actions">
          {detected ? <Pill tone="ok">Read from the game: {detected}</Pill> : <span className="dim" style={{ fontSize: ".86em" }}>Not readable from the game.</span>}
          <select value={patch} onChange={(e) => { setPatch(e.target.value); settings.set("game.patch", e.target.value).catch(() => {}); }} aria-label="Game patch">
            <option value="">{detected ? "Use the game's" : "Pick the patch"}</option>
            {patches.map((x) => <option key={x} value={x}>{x}</option>)}
          </select>
        </div>
        <span className="lab">Mod library</span>
        <div className="actions" style={{ flexWrap: "nowrap" }}>
          <code style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p?.libraryDir ?? "Set the game folder first"}</code>
          <Key size="sm" icon="folder" onClick={async () => { const d = await pickFolder("A folder for downloads and staged mods, on the game's drive"); if (d) save({ libraryDir: d }); }}>Choose</Key>
        </div>
        <span className="lab">Deploy</span>
        <div className="actions">
          {p?.sameVolume === true && <Pill tone="ok">Same drive: hard links</Pill>}
          {p?.sameVolume === false && <Pill tone="over">Different drives</Pill>}
          <Switch checked={!!p?.allowCopy} onChange={(v) => save({ allowCopy: v })}>Copy when hard links are impossible</Switch>
        </div>
      </div>
      {p?.gameDir && !p.gameLooksRight && <div className="note bad" style={{ marginTop: ".7em" }}>That folder has no bin\x64. Pick the folder that holds Cyberpunk2077's bin, archive and r6 folders.</div>}
      {p?.inProgramFiles && <div className="note warn" style={{ marginTop: ".7em" }}>The game is under Program Files, where Windows may block writes. If installs fail with "not allowed", move the game to another Steam library folder (Steam, Properties, Installed files, Move install folder).</div>}
      {p?.sameVolume === false && <div className="note warn" style={{ marginTop: ".7em" }}>The mod library is on another drive, so files would be copied, not linked. Choose a library folder on the game's drive{p.defaultLibrary ? ` such as ${p.defaultLibrary}` : ""}.</div>}

      <div className="sub-h">nxm:// links</div>
      {h && !h.supported && <p className="dim">The "Mod Manager Download" handler is registered on Windows only.</p>}
      {h?.supported && (
        <div className="actions">
          {h.ours ? <Pill tone="ok">Pocket Mods opens nxm:// links</Pill> : <Pill tone="soon">{h.owner ? `${h.owner} opens nxm:// links` : "No app opens nxm:// links"}</Pill>}
          {!h.ours && <Key size="sm" icon="link" onClick={() => handler.register().then((s) => { setH(s); toast("Pocket Mods now opens nxm:// links."); }).catch((e) => toast(errorText(e), true))}>Use Pocket Mods</Key>}
        </div>
      )}
      <p className="dim" style={{ fontSize: ".84em", marginTop: ".4em" }}>Needed only for links clicked in a web browser. Pages opened inside the app hand their links over directly. The setting is per user and needs no admin rights.</p>
    </Module>
  );
}
