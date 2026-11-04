// Settings: version and update check, and the experiments (mod.io read-only browsing).
import { useEffect, useState } from "react";
import { Key, Module, Pill, Switch } from "../ui/pocket";
import { errorText, extras, settings, type AppUpdate } from "../lib/api";
import { toast } from "../ui/toast";

export function About() {
  const [u, setU] = useState<AppUpdate | null>(null);
  const [busy, setBusy] = useState(false);
  return (
    <Module icon="info" title="About" count={u ? `v${u.current}` : undefined}>
      <p>Pocket Mods, a Cyberpunk 2077 mod manager for Nexus Mods.</p>
      <div className="actions" style={{ marginTop: ".6em" }}>
        <Key size="sm" icon="refresh" disabled={busy} onClick={async () => { setBusy(true); try { setU(await extras.appUpdate()); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); } }}>Check for a new version</Key>
        {u && <Pill tone={u.newer ? "soon" : "ok"}>{u.message}</Pill>}
      </div>
      {u?.newer && u.url && <p className="dim" style={{ fontSize: ".86em", marginTop: ".4em" }}>Download it from <code>{u.url}</code>. Updates do not install themselves yet.</p>}
      {u?.notes && <div className="desc" style={{ marginTop: ".4em", maxHeight: "10em", overflow: "auto" }}>{u.notes}</div>}
    </Module>
  );
}

export function Experiments({ onModio }: { onModio?: (on: boolean) => void }) {
  const [on, setOn] = useState(false);
  const [has, setHas] = useState(false);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    settings.get("flags.modio").then((v) => setOn(v === "1")).catch(() => {});
    extras.modioHasKey().then(setHas).catch(() => {});
  }, []);
  return (
    <Module icon="web" title="Experiments" count="off by default">
      <Switch checked={on} onChange={(v) => { setOn(v); onModio?.(v); settings.set("flags.modio", v ? "1" : "0").then(() => window.dispatchEvent(new Event("pm-flags"))).catch(() => {}); }}>Browse mod.io (read-only) in Browse</Switch>
      {on && <>
        <p className="dim" style={{ fontSize: ".86em", margin: ".5em 0" }}>mod.io needs an API key. Whether Cyberpunk 2077's mod.io catalogue is open to other apps is not confirmed. mod.io mods are installed from the game's own Mods menu, not here.</p>
        <div className="actions" style={{ flexWrap: "nowrap" }}>
          <label className="fbox"><input className="finput" type="password" autoComplete="off" value={key} onChange={(e) => setKey(e.target.value)} placeholder={has ? "Replace the saved mod.io key" : "mod.io API key"} aria-label="mod.io API key" /></label>
          <Key size="sm" icon="check" disabled={!key || busy} onClick={async () => {
            setBusy(true);
            try { const g = await extras.modioSetKey(key); setHas(true); setKey(""); toast(g ? `Key saved. Found ${g.name} on mod.io.` : "Key saved, but Cyberpunk 2077 was not found on mod.io."); }
            catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
          }}>Check and save</Key>
          {has && <Key size="sm" variant="ghost" icon="trash" aria-label="Remove the mod.io key" onClick={() => extras.modioClearKey().then(() => setHas(false))} />}
        </div>
      </>}
    </Module>
  );
}
