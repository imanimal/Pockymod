// Play: checks Health first and says what is broken; runs the REDmod deploy if needed; starts the game modded.
import { useState } from "react";
import { Dialog } from "./Dialog";
import { Key } from "./pocket";
import { errorText, game, launchGame, type Issue } from "../lib/api";
import { toast } from "./toast";

export function PlayKey({ onHealth }: { onHealth: () => void }) {
  const [problems, setProblems] = useState<Issue[] | null>(null);
  const [busy, setBusy] = useState(false);
  const start = async () => {
    setBusy(true);
    try {
      const r = await launchGame();
      if (!r.started && r.redmod) toast(`The REDmod deploy reported problems: ${r.redmod.problems[0] ?? "see Load order"}. The game was not started.`, true);
      else toast(r.modded ? "Starting Cyberpunk 2077 with REDmods." : "Starting Cyberpunk 2077.");
      setProblems(null);
    } catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
  };
  const check = async () => {
    setBusy(true);
    try {
      const h = await game.health();
      const bad = h.issues.filter((i) => i.severity === "problem");
      if (bad.length) { setProblems(bad); setBusy(false); return; }
      await start();
    } catch { await start(); }
  };
  return (
    <>
      <Key size="sm" variant="dark" icon="play" onClick={check} disabled={busy} title="Check, then start the game">Play</Key>
      {problems && (
        <Dialog icon="warn" title="Before you play" count={`${problems.length} problems`} band="bad" onClose={() => setProblems(null)}>
          <div className="rows">{problems.slice(0, 5).map((p) => <div key={p.id} className="row" style={{ cursor: "default" }}><span className="m-main"><span className="m-t" style={{ whiteSpace: "normal" }}>{p.title}</span><span style={{ fontSize: ".86em" }}>{p.detail}</span></span></div>)}</div>
          <div className="actions" style={{ justifyContent: "flex-end" }}>
            <Key variant="ghost" disabled={busy} onClick={start}>Play anyway</Key>
            <Key onClick={() => { setProblems(null); onHealth(); }}>Fix them first</Key>
          </div>
        </Dialog>
      )}
    </>
  );
}
