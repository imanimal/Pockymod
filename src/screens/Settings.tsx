import { useEffect, useState } from "react";
import { Key, Lcd, Module, Pill } from "../ui/pocket";
import { errorText, nexus, type NexusStatus } from "../lib/api";
import { Appearance } from "./Appearance";
import { GameFolders } from "./GameFolders";
import { About, Experiments } from "./About";
import { InstallPrefs } from "./InstallPrefs";

export function Settings() {
  return (
    <div className="stack" style={{ maxWidth: "56em" }}>
      <NexusAccount />
      <GameFolders />
      <InstallPrefs />
      <Appearance />
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
