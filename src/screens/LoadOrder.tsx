// Load order: profiles, which mods are on in the active one, the REDmod order, and the REDmod deploy (the screen's
// one orange key).
import { plural } from "../lib/text";
import { useCallback, useEffect, useRef, useState } from "react";
import { Empty, Key, Lcd, Module, Pill, Switch, Tag } from "../ui/pocket";
import { errorText, game, library, profiles, vortexProfiles, type InstallRow, type Profile, type RedmodEntry, type RedmodResult } from "../lib/api";
import { profilesText } from "./Existing";
import { notDeployedText } from "../ui/ProfileSwitch";
import { toast } from "../ui/toast";
import * as ops from "../lib/ops";
import { OverlapsModule, RulesModule } from "./ArchiveOverlaps";

export function LoadOrder() {
  const [vx, setVx] = useState<Awaited<ReturnType<typeof vortexProfiles.scan>> | null>(null);
  const loadVx = useCallback(() => { vortexProfiles.scan().then(setVx).catch(() => setVx(null)); }, []);
  useEffect(loadVx, [loadVx]);
  const [list, setList] = useState<Profile[] | null>(null);
  const [rows, setRows] = useState<InstallRow[] | null>(null);
  const [redmods, setRedmods] = useState<RedmodEntry[]>([]);
  const [result, setResult] = useState<RedmodResult | null>(null);
  // the key at work, so it shows busy while the others wait
  const [working, setWorking] = useState<string | null>(null);
  const busy = working !== null;
  const [name, setName] = useState("");

  // only the newest answers count: a reorder shows at once and an older list must not put it back
  const seq = useRef(0);
  const load = useCallback(() => {
    const n = ++seq.current;
    const fresh = () => n === seq.current;
    profiles.list().then((x) => { if (fresh()) setList(x); }).catch((e) => toast(errorText(e), true));
    library.list().then((x) => { if (fresh()) setRows(x); }).catch((e) => { if (fresh()) { setRows((r) => r ?? []); toast(errorText(e), true); } });
    game.redmodOrder().then((x) => { if (fresh()) setRedmods(x); }).catch(() => {});
  }, []);
  useEffect(() => {
    load();
    let off: (() => void) | undefined, gone = false;
    library.onChanged(load).then((u) => { if (gone) u(); else off = u; });
    return () => { gone = true; off?.(); };
  }, [load]);

  const run = async (key: string, f: () => Promise<unknown>) => { setWorking(key); try { await f(); } catch (e) { toast(errorText(e), true); } finally { setWorking(null); load(); } };
  const move = (i: number, d: -1 | 1) => {
    if (!rows) return;
    const next = [...rows];
    const j = i + d;
    if (j < 0 || j >= next.length) return;
    const before = rows.map((r) => r.id);
    [next[i], next[j]] = [next[j], next[i]];
    setRows(next);
    void run(`move${rows[i].id}${d}`, () => ops.setOrder(before, next.map((r) => r.id), `${rows[i].name} ${d < 0 ? "earlier" : "later"}`));
  };
  const active = list?.find((p) => p.active);
  const copy = () => {
    if (!name.trim() || busy) return;
    void run("copy", async () => { await profiles.create(name, active?.id); setName(""); toast(`Copied ${active?.name ?? "the active profile"} to ${name.trim()}`); });
  };

  return (
    <div className="grid2 lo" style={{ alignItems: "start" }}>
      <div className="stack">
        <Module icon="loadorder" title="REDmod deploy" count={`${redmods.filter((r) => r.enabled).length} REDmods`}>
          <p>Run after adding, removing or reordering REDmods</p>
          <div className="actions" style={{ margin: ".7em 0" }}>
            <Key variant="primary" icon="play" disabled={busy || redmods.length === 0} busy={working === "deploy"} title={redmods.length ? undefined : "No REDmods installed"}
              onClick={() => run("deploy", async () => {
                const r = await game.redmodDeploy();
                setResult(r);
                toast(r.ok ? "REDmod deploy done" : `REDmod deploy hit ${plural(r.problems.length, "problem")}`, !r.ok);
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
            {!list && <span className="skel skel-line" style={{ width: "60%", margin: ".5em" }} aria-hidden="true" />}
            {(list ?? []).map((p) => (
              <div key={p.id} className={`row${p.active ? " sel" : ""}`} style={{ cursor: "default" }}>
                <span className="m-main"><span className="m-t">{p.name}</span><span className="m-meta"><span>{plural(p.enabled, "mod")} on</span>{!!p.vortex && <span title="Installed by Vortex">{p.vortex} from Vortex</span>}{!!p.unmanaged && <span className="warnc" title="Installed by neither manager. Clean up in Library">{p.unmanaged} unmanaged</span>}</span></span>
                <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".4em" }}>
                  {p.active ? <span className="led run" aria-label="Active" title="Active" /> : <>
                    <Key size="sm" disabled={busy} busy={working === `switch${p.id}`}
                      onClick={() => run(`switch${p.id}`, async () => { const t = notDeployedText(await ops.switchProfile(active, p)); if (t) toast(t); })}>Switch</Key>
                    <Key size="sm" variant="ghost" icon="trash" aria-label={`Delete ${p.name}`} title="Delete this profile. Your mods stay installed" disabled={busy} busy={working === `delete${p.id}`}
                      onClick={() => run(`delete${p.id}`, async () => { await profiles.remove(p.id); toast(`Deleted the ${p.name} profile`); })} />
                  </>}
                </span>
              </div>
            ))}
          </div>
          <div className="actions" style={{ marginTop: ".6em", flexWrap: "nowrap" }}>
            <label className="fbox"><input className="finput" value={name} onChange={(e) => setName(e.target.value)} placeholder="New profile name" aria-label="New profile name"
              onKeyDown={(e) => { if (e.key === "Enter") copy(); }} /></label>
            <Key size="sm" icon="plus" disabled={!name.trim() || busy} busy={working === "copy"} onClick={copy}
              title={name.trim() ? "A new profile with the same mods on" : "Type a name first"}>Copy active</Key>
          </div>
          {vx && vx.tracked > 0 && vx.profiles.length > 0 && (
            <div className="actions" style={{ marginTop: ".5em" }}>
              <Key size="sm" variant="ghost" icon="user" disabled={busy} busy={working === "vortex"} onClick={() => run("vortex", async () => { const r = await vortexProfiles.import(); toast(profilesText(r)); loadVx(); })}
                title={`${vx.profiles.map((p) => p.name).join(", ")}, with the same mods on`}>
                {vx.imported ? "Sync with Vortex's profiles again" : `Bring in Vortex's ${vx.profiles.length} profiles`}</Key>
            </div>
          )}
        </Module>

        <OverlapsModule />
        <RulesModule />
      </div>

      <Module icon="library" title="Mods in this profile" count={rows ? `${rows.filter((r) => r.enabled).length} of ${rows.length} on` : undefined} bodyClass="list">
        {rows?.length === 0 && <Empty icon="library" title="No mods installed" />}
        <div className="rows">
          {!rows && Array.from({ length: 5 }, (_, i) => <span key={i} className="skel skel-line" style={{ margin: ".7em .6em", width: `${70 - i * 8}%` }} aria-hidden="true" />)}
          {(rows ?? []).map((r, i, all) => (
            <div key={r.id} className={`row${r.enabled ? "" : " off"}`} style={{ cursor: "default", gridTemplateColumns: "2.2em minmax(0,1fr) auto" }}>
              <span className="mono dim" style={{ fontSize: ".8em", textAlign: "right" }}>{String(i + 1).padStart(2, "0")}</span>
              <span className="m-main">
                <span className="m-t">{r.name}</span>
                <span className="m-tags">{r.modType === "redmod" ? <Tag kind="redmod" square>REDmod</Tag> : <Tag square kind={r.modType === "unknown" || r.modType === "mixed" ? undefined : r.modType}>{r.modType}</Tag>}
                  {!r.enabled && <Pill>Off</Pill>}</span>
              </span>
              <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".3em" }}>
                <Switch checked={r.enabled} onChange={(on) => run(`on${r.id}`, () => ops.setEnabled([r], on))} disabled={busy}><span className="sr-only">On</span></Switch>
                <Key size="sm" variant="ghost" icon="up" aria-label="Earlier" title="Earlier" onClick={() => move(i, -1)} disabled={busy || i === 0} busy={working === `move${r.id}-1`} />
                <Key size="sm" variant="ghost" icon="down" aria-label="Later" title="Later" onClick={() => move(i, 1)} disabled={busy || i === all.length - 1} busy={working === `move${r.id}1`} />
              </span>
            </div>
          ))}
        </div>
        <p className="dim" style={{ fontSize: ".82em", padding: ".6em" }}>Order only affects REDmods. Archives load by file name</p>
      </Module>
    </div>
  );
}
