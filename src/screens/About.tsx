// Settings: version and update check, how the app was made, and the experiments (mod.io read-only browsing).
import { useEffect, useState } from "react";
import { Key, Module, Pill, Switch } from "../ui/pocket";
import { errorText, extras, settings, type AppUpdate } from "../lib/api";
import { toast } from "../ui/toast";

export function About({ onGallery }: { onGallery?: () => void }) {
  const [u, setU] = useState<AppUpdate | null>(null);
  const [busy, setBusy] = useState(false);
  return (
    <Module icon="info" title="About" count={u ? `v${u.current}` : undefined}>
      <div className="actions">
        <Key size="sm" icon="refresh" disabled={busy} onClick={async () => { setBusy(true); try { setU(await extras.appUpdate()); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); } }}>Check for updates</Key>
        {u && <Pill tone={u.newer ? "soon" : "ok"}>{u.message}</Pill>}
        {onGallery && <Key size="sm" variant="ghost" icon="gallery" onClick={onGallery} title="Every UI part in the current theme">Design gallery</Key>}
      </div>
      {u?.newer && u.url && <p className="dim" style={{ fontSize: ".86em", marginTop: ".4em" }}>Get it at <code>{u.url}</code></p>}
      {u?.notes && <div className="desc" style={{ marginTop: ".4em", maxHeight: "10em", overflow: "auto" }}>{u.notes}</div>}
      <MadeWith />
    </Module>
  );
}

/** How the app was made, folded away until asked for. */
function MadeWith() {
  const [open, setOpen] = useState(false);
  return (
    <div style={{ marginTop: ".6em" }}>
      <Key size="sm" variant="ghost" icon="chevron" className={open ? "rot-key" : ""} aria-expanded={open} onClick={() => setOpen(!open)}
        title="How Pockymod was built, and whether it uses AI">How it was made</Key>
      {open && <p className="dim" style={{ fontSize: ".86em", marginTop: ".4em" }}>
        Pockymod was built with an AI coding assistant, working from its developer's direction. The app itself has no
        AI in it: nothing you do here is sent to an AI service.
      </p>}
    </div>
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
    <Module icon="web" title="Experiments" count={on ? "on" : "off"}>
      <Switch checked={on} onChange={(v) => { setOn(v); onModio?.(v); settings.set("flags.modio", v ? "1" : "0").then(() => window.dispatchEvent(new Event("pm-flags"))).catch(() => {}); }}><span title="Read-only. mod.io mods install from the game's Mods menu">mod.io in Browse</span></Switch>
      {on && <>
        <div className="actions" style={{ flexWrap: "nowrap", marginTop: ".5em" }}>
          <label className="fbox"><input className="finput" type="password" autoComplete="off" value={key} onChange={(e) => setKey(e.target.value)} placeholder={has ? "Replace mod.io key" : "mod.io API key"} aria-label="mod.io API key" /></label>
          <Key size="sm" icon="check" disabled={!key || busy} onClick={async () => {
            setBusy(true);
            try { const g = await extras.modioSetKey(key); setHas(true); setKey(""); toast(g ? `Key saved, found ${g.name}` : "Key saved, but Cyberpunk 2077 isn't on mod.io"); }
            catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
          }}>Check and save</Key>
          {has && <Key size="sm" variant="ghost" icon="trash" aria-label="Remove mod.io key" title="Remove key" onClick={() => extras.modioClearKey().then(() => setHas(false))} />}
        </div>
      </>}
    </Module>
  );
}
