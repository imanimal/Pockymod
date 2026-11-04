// The launch deck at the foot of the rail: start the game with mods (after a health check), start it without
// mods (they come out and go back when it closes), and open the game folder.
import { useEffect, useState } from "react";
import { Dialog } from "./Dialog";
import { Key } from "./pocket";
import { PlayKey } from "./PlayKey";
import { errorText, launchVanilla, openGameFolder, vanilla } from "../lib/api";
import { toast } from "./toast";

export function LaunchDeck({ onHealth }: { onHealth: () => void }) {
  const [out, setOut] = useState(false);
  const [ask, setAsk] = useState(false);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    vanilla.pending().then(setOut).catch(() => {});
    let off: (() => void) | undefined;
    vanilla.onChange((e) => {
      setOut(e.state === "running");
      if (e.state === "restored") toast(`The game closed: ${e.back ?? 0} ${e.back === 1 ? "mod is" : "mods are"} back in.`);
    }).then((u) => { off = u; });
    return () => off?.();
  }, []);
  const startVanilla = async () => {
    setAsk(false); setBusy(true);
    try {
      const r = await launchVanilla();
      setOut(true);
      toast(`Starting Cyberpunk 2077 without mods: ${r.out} ${r.out === 1 ? "mod is" : "mods are"} out until the game closes.`);
    } catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
  };
  return (
    <div className="launch" aria-label="Start the game">
      <div className="launch-h"><span className={`led${out ? " wait" : " run"}`} aria-hidden="true" /><span>{out ? "Running without mods" : "Cyberpunk 2077"}</span></div>
      {out ? (
        <Key size="sm" icon="undo" disabled={busy} onClick={() => vanilla.restore().then((n) => { setOut(false); toast(`${n} ${n === 1 ? "mod is" : "mods are"} back in.`); }).catch((e) => toast(errorText(e), true))}
          title="Put the mods back now (they go back by themselves when the game closes)">Put mods back</Key>
      ) : <PlayKey onHealth={onHealth} />}
      <div className="launch-row">
        <Key size="sm" variant="ghost" icon="power" disabled={busy || out} onClick={() => setAsk(true)} title="Start the game with every mod taken out">Without mods</Key>
        <Key size="sm" variant="ghost" icon="folder" onClick={() => openGameFolder().catch((e) => toast(errorText(e), true))} aria-label="Open the game folder" title="Open the game folder" />
      </div>
      {ask && (
        <Dialog icon="power" title="Start without mods" onClose={() => setAsk(false)}>
          <p>Every mod Pocket Mods tracks comes out of the game folder, the game starts, and they all go back when you close it. It takes a moment either way: they are hard links, nothing is copied.</p>
          <p className="dim" style={{ fontSize: ".86em" }}>Useful to check whether a problem comes from a mod. Mods put in by hand that are not in your library stay in; bring them in from Library first to take them out too.</p>
          <div className="actions" style={{ justifyContent: "flex-end" }}>
            <Key variant="ghost" onClick={() => setAsk(false)}>Cancel</Key>
            <Key variant="dark" icon="power" onClick={startVanilla}>Start without mods</Key>
          </div>
        </Dialog>
      )}
    </div>
  );
}
