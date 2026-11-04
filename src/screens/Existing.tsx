// "Find existing mods": what Vortex installed and what is already in the game folder, tracked in one click.
import { useEffect, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Empty, Key, Lcd, Switch } from "../ui/pocket";
import { errorText, existing, updates, type ExistingScan } from "../lib/api";
import { toast } from "../ui/toast";

export function ExistingBody({ onDone, compact, quiet }: { onDone: (tracked: number) => void; compact?: boolean; quiet?: boolean }) {
  const [scan, setScan] = useState<ExistingScan | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [vx, setVx] = useState(true);
  const [fd, setFd] = useState(true);
  const [busy, setBusy] = useState(false);
  useEffect(() => { existing.scan().then(setScan).catch((e) => setErr(errorText(e))); }, []);
  if (err) return <div className="note bad">{err}</div>;
  if (!scan) return <Empty icon="clock" title="Looking in the game folder" />;
  const v = scan.vortex;
  const newVortex = v ? v.mods - v.alreadyTracked : 0;
  const nothing = newVortex === 0 && scan.found.length === 0;
  const go = async () => {
    setBusy(true);
    try {
      const r = await existing.track(vx && newVortex > 0, fd && scan.found.length > 0);
      const n = r.vortex.tracked + r.found;
      // matched mods get their update badges right away
      void updates.check().catch(() => {});
      toast(`${n} ${n === 1 ? "mod is" : "mods are"} now in your library${r.vortex.tracked ? `, ${r.vortex.matchedOnNexus} matched on Nexus for updates` : ""}.`);
      onDone(n);
    } catch (e) { toast(errorText(e), true); setBusy(false); }
  };
  if (nothing) return <><Empty icon="check" title="Nothing new to bring in">Every mod in the game folder is already in your library.</Empty>{!compact && <div className="actions" style={{ justifyContent: "flex-end" }}><Key onClick={() => onDone(0)}>Done</Key></div>}</>;
  return (
    <>
      <div className="lcds">
        {v && <Lcd size="sm" label="From Vortex" value={String(newVortex).padStart(2, "0")} sub={`${v.deployed} in the game, ${v.mods - v.deployed} turned off`} />}
        <Lcd size="sm" label="Put in by hand" value={String(scan.found.length).padStart(2, "0")} sub="no manager owns them" />
      </div>
      {v && newVortex > 0 && <>
        <Switch checked={vx} onChange={setVx}>Bring in Vortex's mods</Switch>
        <p className="dim" style={{ fontSize: ".86em" }}>
          They show and update like any other mod and stay Vortex's: updates go into Vortex's own folders and its records, so Vortex still works with them.
          Nothing is moved. {v.sample.length > 0 && <>For example: {v.sample.slice(0, 4).join(", ")}{v.mods > 4 ? "…" : ""}.</>}
        </p>
      </>}
      {scan.found.length > 0 && <>
        <Switch checked={fd} onChange={setFd}>Bring in mods that are in the game folder already</Switch>
        <p className="dim" style={{ fontSize: ".86em" }}>{scan.found.slice(0, 5).map((m) => m.name).join(", ")}{scan.found.length > 5 ? ` and ${scan.found.length - 5} more` : ""}. Their files stay where they are; the library keeps a linked copy so turning them off is reversible.</p>
      </>}
      <div className="actions" style={{ justifyContent: "flex-end" }}>
        {!compact && <Key variant="ghost" onClick={() => onDone(0)}>Not now</Key>}
        <Key variant={quiet ? "dark" : "primary"} icon="library" disabled={busy || (!vx && !fd)} onClick={go}>{busy ? "Bringing them in" : "Bring them in"}</Key>
      </div>
    </>
  );
}

export function ExistingDialog({ onClose }: { onClose: () => void }) {
  return <Dialog icon="folder" title="Find existing mods" onClose={onClose}><ExistingBody onDone={onClose} /></Dialog>;
}
