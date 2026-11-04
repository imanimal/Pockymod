// Load order: profiles, which mods are on in the active one, the REDmod order, and the REDmod deploy (the screen's
// one orange key).
import { plural } from "../lib/text";
import { useCallback, useEffect, useState } from "react";
import { Empty, Key, Lcd, Module, Pill, Switch, Tag } from "../ui/pocket";
import { errorText, game, library, profiles, vortexProfiles, type InstallRow, type Profile, type RedmodEntry, type RedmodResult } from "../lib/api";
import { profilesText } from "./Existing";
import { toast } from "../ui/toast";
import * as ops from "../lib/ops";
import { OverlapsModule, RulesModule } from "./ArchiveOverlaps";

export function LoadOrder() {
  const [vx, setVx] = useState<Awaited<ReturnType<typeof vortexProfiles.scan>> | null>(null);
  useEffect(() => { vortexProfiles.scan().then(setVx).catch(() => setVx(null)); }, []);
  const [list, setList] = useState<Profile[]>([]);
  const [rows, setRows] = useState<InstallRow[]>([]);
  const [redmods, setRedmods] = useState<RedmodEntry[]>([]);
  const [result, setResult] = useState<RedmodResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState("");

  const load = useCallback(() => {
    profiles.list().then(setList).catch((e) => toast(errorText(e), true));
    library.list().then(setRows).catch(() => {});
    game.redmodOrder().then(setRedmods).catch(() => {});
  }, []);
  useEffect(() => {
    load();
    let off: (() => void) | undefined;
    library.onChanged(load).then((u) => { off = u; });
    return () => off?.();
  }, [load]);

  const run = async (f: () => Promise<unknown>) => { setBusy(true); try { await f(); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); load(); } };
  const move = (i: number, d: -1 | 1) => {
    const next = [...rows];
    const j = i + d;
    if (j < 0 || j >= next.length) return;
    const before = rows.map((r) => r.id);
    [next[i], next[j]] = [next[j], next[i]];
    setRows(next);
    void run(() => ops.setOrder(before, next.map((r) => r.id), `${rows[i].name} ${d < 0 ? "earlier" : "later"}`));
  };
  const active = list.find((p) => p.active);

  return (
    <div className="grid2 lo" style={{ alignItems: "start" }}>
      <div className="stack">
        <Module icon="loadorder" title="REDmod deploy" count={`${redmods.filter((r) => r.enabled).length} REDmods`}>
          <p>The game only sees REDmods after its REDmod tool compiles them, in the order below. Run it after adding, removing or reordering REDmods.</p>
          <div className="actions" style={{ margin: ".7em 0" }}>
            <Key variant="primary" icon="play" disabled={busy || redmods.length === 0} onClick={() => run(async () => {
              const r = await game.redmodDeploy();
              setResult(r);
              toast(r.ok ? "REDmod deploy done." : `REDmod deploy reported ${plural(r.problems.length, "problem")}.`, !r.ok);
            })}>Run REDmod deploy</Key>
          </div>
          {result && <>
            <Lcd size="sm" label="Last deploy" value={result.ok ? "OK" : String(result.problems.length).padStart(2, "0")} tone={result.ok ? "ok" : "bad"} sub={result.ok ? `${plural(result.mods.length, "mod")} compiled` : "problems"} />
            {result.problems.map((p) => <div key={p} className="note bad" style={{ marginTop: ".4em" }}>{p}</div>)}
            {result.log && <div className="log">{result.log}</div>}
          </>}
        </Module>

        <Module icon="user" title="Profiles" count={active ? `${active.name} on` : undefined}>
          <div className="rows">
            {list.map((p) => (
              <div key={p.id} className={`row${p.active ? " sel" : ""}`} style={{ cursor: "default" }}>
                <span className="m-main"><span className="m-t">{p.name}</span><span className="m-meta"><span>{plural(p.enabled, "mod")} on</span>{!!p.vortex && <span title="Mods Vortex installed, as Vortex lists them in this profile">{p.vortex} from Vortex</span>}{!!p.unmanaged && <span className="warnc" title="Mods that were already in the game folder, put there by neither Vortex nor Pockymod (by hand, or left behind). Library, Unmanaged mods, cleans them up.">{p.unmanaged} unmanaged</span>}</span></span>
                <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".4em" }}>
                  {p.active ? <span className="led run" aria-label="Active" /> : <>
                    <Key size="sm" onClick={() => run(async () => { const r = await ops.switchProfile(active, p); if (r.notDeployed.length) toast(`${plural(r.notDeployed.length, "mod")} need${r.notDeployed.length === 1 ? "s" : ""} a decision in Library.`); })} disabled={busy}>Switch</Key>
                    <Key size="sm" variant="ghost" icon="trash" aria-label={`Delete ${p.name}`} onClick={() => run(() => profiles.remove(p.id))} disabled={busy} />
                  </>}
                </span>
              </div>
            ))}
          </div>
          <div className="actions" style={{ marginTop: ".6em", flexWrap: "nowrap" }}>
            <label className="fbox"><input className="finput" value={name} onChange={(e) => setName(e.target.value)} placeholder="New profile name" aria-label="New profile name" /></label>
            <Key size="sm" icon="plus" disabled={!name.trim() || busy} onClick={() => run(async () => { await profiles.create(name, active?.id); setName(""); toast("Profile made as a copy of the active one."); })}>Copy active</Key>
          </div>
          {vx && vx.tracked > 0 && vx.profiles.length > 0 && (
            <div className="actions" style={{ marginTop: ".5em" }}>
              <Key size="sm" variant="ghost" icon="user" disabled={busy} onClick={() => run(async () => { const r = await vortexProfiles.import(); toast(profilesText(r)); })}
                title={`Vortex's profiles (${vx.profiles.map((p) => p.name).join(", ")}) as profiles here, with exactly the mods Vortex has on in each; unmanaged mods stay only in the profile that is live`}>
                {vx.imported ? "Sync with Vortex's profiles again" : `Bring in Vortex's ${vx.profiles.length} profiles`}</Key>
            </div>
          )}
        </Module>

        <OverlapsModule />
        <RulesModule />
      </div>

      <Module icon="library" title="Mods in this profile" count={`${rows.filter((r) => r.enabled).length} of ${rows.length} on`} bodyClass="list">
        {rows.length === 0 && <Empty icon="library" title="No mods installed" />}
        <div className="rows">
          {rows.map((r, i) => (
            <div key={r.id} className={`row${r.enabled ? "" : " off"}`} style={{ cursor: "default", gridTemplateColumns: "2.2em minmax(0,1fr) auto" }}>
              <span className="mono dim" style={{ fontSize: ".8em", textAlign: "right" }}>{String(i + 1).padStart(2, "0")}</span>
              <span className="m-main">
                <span className="m-t">{r.name}</span>
                <span className="m-tags">{r.modType === "redmod" ? <Tag kind="redmod" square>REDmod</Tag> : <Tag square kind={r.modType === "unknown" || r.modType === "mixed" ? undefined : r.modType}>{r.modType}</Tag>}
                  {!r.enabled && <Pill>Off</Pill>}</span>
              </span>
              <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".3em" }}>
                <Switch checked={r.enabled} onChange={(on) => run(() => ops.setEnabled([r], on))} disabled={busy}><span className="sr-only">On</span></Switch>
                <Key size="sm" variant="ghost" icon="up" aria-label="Earlier" onClick={() => move(i, -1)} disabled={busy || i === 0} />
                <Key size="sm" variant="ghost" icon="down" aria-label="Later" onClick={() => move(i, 1)} disabled={busy || i === rows.length - 1} />
              </span>
            </div>
          ))}
        </div>
        <p className="dim" style={{ fontSize: ".82em", padding: ".6em" }}>The order matters for REDmods (they are compiled first to last). Archive mods load by file name, whatever their place here: see Overlapping archives.</p>
      </Module>
    </div>
  );
}
