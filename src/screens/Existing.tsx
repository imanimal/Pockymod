// "Find existing mods": what Vortex installed and what is already in the game folder, tracked in one click.
import { useEffect, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Empty, Key, Lcd, Switch } from "../ui/pocket";
import { Scanner, type ScanLine } from "../ui/Scanner";
import { errorText, existing, onExistingProgress, updates, type ExistingProgress, type ExistingScan, type ProfileImport } from "../lib/api";
import { toast } from "../ui/toast";

/** The progress events as readable lines: each folder looked in, each Vortex mod matched on Nexus. */
function useProgress() {
  const [lines, setLines] = useState<ScanLine[]>([]);
  const [p, setP] = useState<ExistingProgress | null>(null);
  useEffect(() => {
    let off: (() => void) | undefined;
    onExistingProgress((e) => {
      setP(e);
      const text = e.stage === "vortex" ? `Vortex: ${e.name}` : e.stage === "folder" ? e.name
        : e.stage === "matching" ? (e.name ? `Matched on Nexus: ${e.name}` : "Matching Vortex's mods on Nexus")
        : e.stage === "tracking" ? "Recording them in your library" : e.stage === "profiles" ? "Bringing in Vortex's profiles" : "Linking hand-installed mods into the library";
      setLines((l) => [...l.map((x) => ({ ...x, done: true })), { text }].slice(-40));
    }).then((u) => { off = u; });
    return () => off?.();
  }, []);
  return { lines, p, reset: () => { setLines([]); setP(null); } };
}

export function ExistingBody({ onDone, compact, quiet }: { onDone: (tracked: number) => void; compact?: boolean; quiet?: boolean }) {
  const [scan, setScan] = useState<ExistingScan | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [vx, setVx] = useState(true);
  const [fd, setFd] = useState(true);
  const [px, setPx] = useState(true);
  const [busy, setBusy] = useState(false);
  const prog = useProgress();
  useEffect(() => { existing.scan().then(setScan).catch((e) => setErr(errorText(e))); }, []);
  if (err) return <div className="note bad">{err}</div>;
  if (!scan) return <Scanner title="Looking for mods you already have" icon="folder" lines={prog.lines} sub="Vortex's records and the game's mod folders. Nothing is changed." />;
  if (busy) {
    const matching = prog.p?.stage === "matching";
    return <Scanner title="Bringing your mods in" icon="library" lines={prog.lines} done={matching ? prog.p?.done : undefined} total={matching ? prog.p?.total : undefined}
      sub={matching ? "Each Vortex mod is looked up once on Nexus so it can get update checks; next time this is instant." : "Nothing in the game folder moves."} />;
  }
  const v = scan.vortex;
  const newVortex = v ? v.mods - v.alreadyTracked : 0;
  const nothing = newVortex === 0 && scan.found.length === 0;
  const go = async () => {
    setBusy(true);
    prog.reset();
    try {
      const r = await existing.track(vx && newVortex > 0, fd && scan.found.length > 0, px);
      const n = r.vortex.tracked + r.found;
      // matched mods get their update badges right away
      void updates.check().catch(() => {});
      toast(`${n} ${n === 1 ? "mod is" : "mods are"} now in your library${r.vortex.tracked ? `, ${r.vortex.matchedOnNexus} matched on Nexus for updates` : ""}.`);
      if (r.profiles) toast(profilesText(r.profiles));
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
        {vx && v.profiles.length > 1 && <>
          <Switch checked={px} onChange={setPx}>And its {v.profiles.length} profiles: {v.profiles.map((p) => p.name).join(", ")}</Switch>
          <p className="dim" style={{ fontSize: ".86em" }}>Each becomes a profile here with the same mods on. {v.profiles.find((p) => p.active)?.name ?? "The one Vortex uses now"} stays the active one, so the game does not change.</p>
        </>}
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

/** What bringing in Vortex's profiles did, in one line. */
export function profilesText(r: ProfileImport): string {
  const made = r.profiles.filter((p) => p.created).length;
  const names = r.profiles.map((p) => p.name).join(", ");
  const moved = r.deployed + r.removed;
  return `Vortex's profiles are here: ${names}${made < r.profiles.length ? ` (${made} new, ${r.profiles.length - made} updated)` : ""}. ${r.active ?? "The active one"} is active${moved ? `; ${moved} ${moved === 1 ? "mod" : "mods"} changed in the game to match it` : ", nothing in the game changed"}.${r.notDeployed.length ? ` ${r.notDeployed.length} need a decision in Library.` : ""}`;
}

export function ExistingDialog({ onClose }: { onClose: () => void }) {
  return <Dialog icon="folder" title="Find existing mods" onClose={onClose}><ExistingBody onDone={onClose} /></Dialog>;
}
