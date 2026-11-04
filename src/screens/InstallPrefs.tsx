// Settings: how Install behaves.
import { useEffect, useState } from "react";
import { Module, Seg, Switch } from "../ui/pocket";
import { installPrefs, setInstallPrefs, type InstallPrefs as Prefs, type Requirements } from "../lib/prefs";

export function InstallPrefs() {
  const [p, setP] = useState<Prefs | null>(null);
  useEffect(() => {
    const load = () => installPrefs().then(setP).catch(() => {});
    load();
    window.addEventListener("pm-prefs", load);
    return () => window.removeEventListener("pm-prefs", load);
  }, []);
  if (!p) return null;
  const save = (x: Partial<Prefs>) => { setP({ ...p, ...x }); void setInstallPrefs(x); };
  return (
    <Module icon="install" title="Installing" count={p.confirm ? "shows the plan" : "one click"}>
      <div className="fgrid">
        <span className="lab">Required mods</span>
        <Seg<Requirements> label="Required mods" value={p.requirements} onChange={(v) => save({ requirements: v })}
          options={[{ value: "always", label: "Add them" }, { value: "ask", label: "Ask me" }, { value: "never", label: "Leave them out" }]} />
        <span className="lab">Before installing</span>
        <Switch checked={p.confirm} onChange={(v) => save({ confirm: v })}>Show what will be installed, the clicks and the size</Switch>
      </div>
      <p className="dim" style={{ fontSize: ".84em", marginTop: ".5em" }}>With the plan off and required mods added, Install is one click: the mod and everything it needs go straight to the queue.</p>
    </Module>
  );
}
