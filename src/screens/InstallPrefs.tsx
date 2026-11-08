// Settings: how Install behaves.
import { useEffect, useState } from "react";
import { Module, Seg, Switch } from "../ui/pocket";
import { installPrefs, setInstallPrefs, type InstallPrefs as Prefs, type Requirements } from "../lib/prefs";
import { errorText } from "../lib/api";
import { toast } from "../ui/toast";

const load = (set: (p: Prefs) => void) => installPrefs().then(set).catch(() => {});

export function InstallPrefs() {
  const [p, setP] = useState<Prefs | null>(null);
  useEffect(() => {
    const h = () => load(setP);
    h();
    window.addEventListener("pm-prefs", h);
    return () => window.removeEventListener("pm-prefs", h);
  }, []);
  if (!p) return null;
  // shown at once; if it does not save, say so and show what is saved
  const save = (x: Partial<Prefs>) => { setP({ ...p, ...x }); setInstallPrefs(x).catch((e) => { toast(errorText(e), true); void load(setP); }); };
  return (
    <Module icon="install" title="Installing" count={p.confirm ? "shows plan" : "one click"}>
      <div className="fgrid">
        <span className="lab">Required mods</span>
        <Seg<Requirements> label="Required mods" value={p.requirements} onChange={(v) => save({ requirements: v })}
          options={[{ value: "always", label: "Add them" }, { value: "ask", label: "Ask me" }, { value: "never", label: "Leave them out" }]} />
        <span className="lab">Before installing</span>
        <Switch checked={p.confirm} onChange={(v) => save({ confirm: v })}><span title="What installs, clicks needed and size">Show the plan first</span></Switch>
      </div>
    </Module>
  );
}
