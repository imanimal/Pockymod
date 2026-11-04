import { useEffect, useState } from "react";
import { Key, Lcd, Module, Pill, Seg, Switch } from "../ui/pocket";
import { applyTheme, getThemeChoice, setMotion, setThemeChoice, type ThemeChoice } from "../lib/theme";
import { errorText, nexus, type NexusStatus } from "../lib/api";
import { GameFolders } from "./GameFolders";
import { About, Experiments } from "./About";

export function Settings({ onScheme }: { scheme: "cream" | "night"; onScheme: (s: "cream" | "night") => void }) {
  const [theme, setTheme] = useState<ThemeChoice>(getThemeChoice());
  const [motion, setMotionState] = useState(document.documentElement.dataset.motion !== "0");

  return (
    <div className="stack" style={{ maxWidth: "56em" }}>
      <NexusAccount />
      <GameFolders />
      <Module icon="settings" title="Look" count="Pocket">
        <div className="fgrid">
          <span className="lab">Theme</span>
          <Seg label="Theme" value={theme} onChange={(t) => { setTheme(t); setThemeChoice(t); onScheme(applyTheme(t)); }}
            options={[{ value: "system", label: "System" }, { value: "cream", label: "Cream" }, { value: "night", label: "Night" }]} />
          <span className="lab">Motion</span>
          <Switch checked={motion} onChange={(v) => { setMotionState(v); setMotion(v); }}>Keys travel and LCDs flicker</Switch>
        </div>
      </Module>
      <Experiments />
      <About />
    </div>
  );
}

function NexusAccount() {
  const [st, setSt] = useState<NexusStatus | null>(null);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => { nexus.status().then(setSt).catch((e) => setErr(errorText(e))); }, []);

  const save = async () => {
    setBusy(true); setErr(null);
    try { setSt(await nexus.setKey(key)); setKey(""); } catch (e) { setErr(errorText(e)); } finally { setBusy(false); }
  };
  const clear = async () => {
    setBusy(true); setErr(null);
    try { setSt(await nexus.clearKey()); } catch (e) { setErr(errorText(e)); } finally { setBusy(false); }
  };
  const user = st?.user;
  const rl = st?.rateLimit;

  return (
    <Module icon="key" title="Nexus account" count={user ? user.name : st?.hasKey ? "key saved" : "not connected"}>
      <div className="grid2" style={{ alignItems: "start" }}>
        <div className="stack" style={{ gap: ".7em" }}>
          {user ? (
            <div className="actions">
              <b className="hi">{user.name}</b>
              {user.isPremium ? <Pill tone="ok">Premium</Pill> : <Pill>Free account</Pill>}
              {user.isSupporter && <Pill>Supporter</Pill>}
            </div>
          ) : (
            <p>Browsing works without an account. Downloads, endorsing and tracking need your personal API key from the
              Nexus site (Site preferences, API keys).</p>
          )}
          <label className="fbox" style={{ flex: "none" }}>
            <input className="finput" type="password" autoComplete="off" spellCheck={false} placeholder={st?.hasKey ? "Replace the saved key" : "Paste your personal API key"}
              value={key} onChange={(e) => setKey(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter" && key) void save(); }} aria-label="Nexus API key" />
          </label>
          <div className="actions">
            <Key icon="check" onClick={save} disabled={busy || !key}>Check and save</Key>
            {st?.hasKey && <Key variant="ghost" icon="trash" onClick={clear} disabled={busy}>Remove key</Key>}
          </div>
          {err && <div className="note bad">{err}</div>}
          <p className="dim" style={{ fontSize: ".84em" }}>The key is kept in Windows Credential Manager, never in a file. A personal key is for your own use.</p>
        </div>
        {rl && (
          <Lcd label="Requests left" tone={rl.level === "throttled" ? "bad" : rl.level === "low" ? "hot" : "ok"}
            value={rl.remaining ?? "---"} unit={rl.limit ? `/${rl.limit}` : undefined}
            sub={rl.level === "unknown" ? "Nexus reports this after the first keyed request" : `This ${rl.window}${rl.level === "low" ? ", slowing down" : rl.level === "throttled" ? ", throttled" : ""}`}
            level={rl.limit && rl.remaining !== null ? { on: Math.ceil((rl.remaining / rl.limit) * 10), of: 10 } : undefined} />
        )}
      </div>
    </Module>
  );
}
