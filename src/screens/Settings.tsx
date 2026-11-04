import { useEffect, useState } from "react";
import { Key, Lcd, Module, Pill, Seg } from "../ui/pocket";
import { background, browserPane, errorText, handler, nexus, nexusPages, pagesWhere, settings, type BackgroundState, type HandlerStatus, type NexusStatus, type PagesWhere } from "../lib/api";
import { Switch } from "../ui/pocket";
import { toast } from "../ui/toast";
import { API_KEY_PAGE } from "./Onboarding";
import { Appearance } from "./Appearance";
import { GameFolders } from "./GameFolders";
import { About, Experiments } from "./About";
import { InstallPrefs } from "./InstallPrefs";

export function Settings({ onGallery }: { onGallery?: () => void }) {
  return (
    <div className="cols">
      <NexusAccount />
      <NexusPagesModule />
      <BackgroundModule />
      <GameFolders />
      <InstallPrefs />
      <Appearance />
      <Experiments />
      <About onGallery={onGallery} />
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
            {!st?.hasKey && <Key variant="ghost" icon="web" onClick={() => nexusPages.url(API_KEY_PAGE).catch((e) => setErr(errorText(e)))}>Get my key</Key>}
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

/** Where Nexus pages open: the user's own browser (the default) or the app's browser pane. */
function NexusPagesModule() {
  const [where, setWhere] = useState<PagesWhere | null>(null);
  const [h, setH] = useState<HandlerStatus | null>(null);
  const [ext, setExt] = useState<[string, number] | null>(null);
  useEffect(() => {
    pagesWhere.get().then(setWhere).catch(() => setWhere("browser"));
    handler.status().then(setH).catch(() => {});
    browserPane.extensionsFolder(false).then(setExt).catch(() => {});
  }, []);
  if (!where) return null;
  const pick = (w: PagesWhere) => { setWhere(w); void pagesWhere.set(w).catch((e) => toast(errorText(e), true)); };
  const takeBack = () => handler.register().then((x) => { setH(x); toast("Mod Manager Download links from your browser open in Pockymod again."); }).catch((e) => toast(errorText(e), true));
  return (
    <Module icon="web" title="Nexus pages" count={where === "browser" ? "in your browser" : "in the app"}>
      <Seg<PagesWhere> label="Open Nexus pages" value={where} onChange={pick}
        options={[{ value: "browser", label: "In your browser" }, { value: "pane", label: "In the app" }]} />
      {where === "browser" ? (
        <p style={{ marginTop: ".5em" }}>Pages open in your own browser, where you are already signed in to Nexus and your saved passwords and
          extensions work. Mod Manager Download there comes straight back to the queue here.</p>
      ) : (
        <p style={{ marginTop: ".5em" }}>Pages open in a pane beside your mods: drag its edge to resize it, or put it over the whole view, behind
          a tab or in a window of its own. Sign in to Nexus there once and it stays signed in. Your browser's saved passwords are not
          in it; the pane's link key opens any page in your browser.</p>
      )}
      {h?.supported && (h.ours
        ? <p className="dim" style={{ fontSize: ".86em", marginTop: ".4em" }}><Pill tone="ok">Linked</Pill> Mod Manager Download links from your browser come to Pockymod.</p>
        : <div className="note warn" style={{ marginTop: ".5em" }}>Mod Manager Download links from your browser go to {h.owner ?? "another program"}, so downloads started there do not reach the queue.
            <div className="actions" style={{ marginTop: ".4em" }}><Key size="sm" icon="link" onClick={takeBack}>Take them back</Key></div></div>)}
      {where === "pane" && ext && (
        <>
          <div className="sub-h">Extensions in the pane</div>
          <p className="dim" style={{ fontSize: ".86em" }}>Unpacked Chrome extensions (a folder with a manifest.json each) in the pane's extensions folder load when the
            pane first opens; restart the app after adding one. {ext[1] ? `${ext[1]} found.` : "None yet."} Not every extension works in the pane.</p>
          <div className="actions" style={{ marginTop: ".3em" }}><Key size="sm" variant="ghost" icon="folder" onClick={() => browserPane.extensionsFolder(true).then(setExt).catch((e) => toast(errorText(e), true))}>Open the extensions folder</Key></div>
        </>
      )}
    </Module>
  );
}

/** Running in the background and quiet updates: each one a choice, on by default except starting with Windows and
 *  installing mod updates by themselves. */
function BackgroundModule() {
  const [v, setV] = useState<Record<string, string> | null>(null);
  const [auto, setAuto] = useState(false);
  const [st, setSt] = useState<BackgroundState | null>(null);
  const [busy, setBusy] = useState(false);
  const KEYS = { bg: "app.background", up: "app.updates", pre: "mods.prefetch", inst: "mods.autoInstall" } as const;
  useEffect(() => {
    Promise.all(Object.values(KEYS).map((k) => settings.get(k).catch(() => null))).then(([bg, up, pre, inst]) =>
      setV({ bg: bg ?? "1", up: up ?? "auto", pre: pre ?? "1", inst: inst ?? "0" }));
    background.autostart().then(setAuto).catch(() => {});
    background.state().then(setSt).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  if (!v) return null;
  const put = (k: keyof typeof KEYS, val: string) => { setV({ ...v, [k]: val }); void settings.set(KEYS[k], val).catch((e) => toast(errorText(e), true)); };
  return (
    <Module icon="power" title="Background" count={v.bg === "1" ? "on" : "off"}>
      <div className="stack" style={{ gap: ".55em" }}>
        <Switch checked={v.bg === "1"} onChange={(x) => put("bg", x ? "1" : "0")}>
          <span title="The window closes, the app stays in the tray: downloads, browser links and update checks carry on, and nothing is drawn, so the game does not feel it. Quit from the tray.">Keep running in the tray when I close the window</span>
        </Switch>
        <Switch checked={auto} onChange={(x) => background.setAutostart(x).then(setAuto).catch((e) => toast(errorText(e), true))}>
          <span title="Starts with Windows in the tray, without a window, so updates are ready before you open it.">Start with Windows, in the tray</span>
        </Switch>
        <div className="sub-h">New versions of Pockymod</div>
        <Seg label="App updates" value={v.up} onChange={(x) => put("up", x)}
          options={[{ value: "auto", label: "Install when I'm away" }, { value: "ask", label: "Download, I'll restart" }, { value: "off", label: "Off" }]} />
        <p className="dim" style={{ fontSize: ".84em" }}>{v.up === "auto" ? "A new version downloads quietly and goes in when the window is closed or you quit, never during a download. Next time you open the app, it is just newer."
          : v.up === "ask" ? "A new version downloads quietly; the tray and this screen offer Restart into it." : "No checks for new versions."}</p>
        <div className="sub-h">Mod updates</div>
        <Switch checked={v.pre === "1"} onChange={(x) => put("pre", x ? "1" : "0")}>
          <span title="Premium accounts: the files of mod updates download in the background, so Update all is instant. Free accounts need a click on Nexus for each file, so nothing downloads by itself.">Download mod updates ahead (Premium)</span>
        </Switch>
        <Switch checked={v.inst === "1"} onChange={(x) => put("inst", x ? "1" : "0")}>
          <span title="Downloaded mod updates go in by themselves, only while the window is closed and the game is not running. Off: they wait for Update all.">Install them by themselves when I'm away</span>
        </Switch>
        <div className="actions" style={{ marginTop: ".3em" }}>
          <Key size="sm" icon="refresh" disabled={busy} onClick={() => { setBusy(true); background.check().then(setSt).catch((e) => toast(errorText(e), true)).finally(() => setBusy(false)); }}
            title="Look for mod updates and a new Pockymod now">Check now</Key>
          {st?.readyVersion && <Key size="sm" variant="dark" icon="power" onClick={() => background.installUpdate().catch((e) => toast(errorText(e), true))}
            title="Close, install the new version and open it again">Restart into {st.readyVersion}</Key>}
          {st && st.modsReady > 0 && <span className="dim" style={{ fontSize: ".84em" }}>{st.modsReady} mod {st.modsReady === 1 ? "update is" : "updates are"} downloaded and ready.</span>}
        </div>
      </div>
    </Module>
  );
}
