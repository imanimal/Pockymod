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
      if (e.state === "restored") toast(`Game closed, ${e.back ?? 0} ${e.back === 1 ? "mod" : "mods"} back in`);
    }).then((u) => { off = u; });
    return () => off?.();
  }, []);
  const startVanilla = async () => {
    setAsk(false); setBusy(true);
    try {
      const r = await launchVanilla();
      setOut(true);
      toast(`Starting without mods, ${r.out} out until the game closes`);
    } catch (e) { toast(errorText(e), true); vanilla.pending().then(setOut).catch(() => {}); } finally { setBusy(false); }
  };
  return (
    <div className="launch" aria-label="Start the game">
      <div className="launch-h"><span className={`led${out ? " wait" : " run"}`} aria-hidden="true" /><span>{out ? "Running without mods" : "Cyberpunk 2077"}</span></div>
      {out ? (
        <Key size="sm" icon="undo" disabled={busy} onClick={() => vanilla.restore().then((n) => { setOut(false); toast(`${n} ${n === 1 ? "mod" : "mods"} back in`); }).catch((e) => toast(errorText(e), true))}
          title="Otherwise they return when the game closes">Put mods back</Key>
      ) : <PlayKey onHealth={onHealth} />}
      <div className="launch-row">
        <Key size="sm" variant="ghost" icon="power" disabled={busy || out} onClick={() => setAsk(true)} title="Start the game with all mods out">Without mods</Key>
        <Key size="sm" variant="ghost" icon="folder" onClick={() => openGameFolder().catch((e) => toast(errorText(e), true))} aria-label="Open the game folder" title="Open the game folder" />
      </div>
      {ask && (
        <Dialog icon="power" title="Start without mods" onClose={() => setAsk(false)}>
          <p>Your mods come out until the game closes</p>
          <p className="dim" style={{ fontSize: ".86em" }}>Mods added by hand outside your library stay in</p>
          <div className="actions" style={{ justifyContent: "flex-end" }}>
            <Key variant="ghost" onClick={() => setAsk(false)}>Cancel</Key>
            <Key variant="dark" icon="power" onClick={startVanilla}>Start without mods</Key>
          </div>
        </Dialog>
      )}
    </div>
  );
}
