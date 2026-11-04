// Settings: where the game and the mod library are, the copy fallback, and the nxm:// handler.
import { useEffect, useState } from "react";
import { Key, Module, Pill, Switch } from "../ui/pocket";
import { errorText, handler, paths, pickFolder, type HandlerStatus, type PathsView } from "../lib/api";
import { toast } from "../ui/toast";

export function GameFolders() {
  const [p, setP] = useState<PathsView | null>(null);
  const [h, setH] = useState<HandlerStatus | null>(null);
  useEffect(() => {
    paths.get().then(setP).catch((e) => toast(errorText(e), true));
    handler.status().then(setH).catch(() => {});
  }, []);
  const save = (x: Parameters<typeof paths.set>[0]) => paths.set(x).then(setP).catch((e) => toast(errorText(e), true));

  return (
    <Module icon="folder" title="Game and mod folders" count={p?.gameLooksRight ? "game found" : "not set"}>
      <div className="fgrid">
        <span className="lab">Cyberpunk 2077</span>
        <div className="actions" style={{ flexWrap: "nowrap" }}>
          <code style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p?.gameDir ?? "Not set"}</code>
          <Key size="sm" icon="folder" onClick={async () => { const d = await pickFolder("The Cyberpunk 2077 folder (it holds bin\\x64)"); if (d) save({ gameDir: d }); }}>Choose</Key>
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
