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
import { PREMIUM_TIPS, getPremium } from "../ui/Premium";
import { NexusSignIn } from "../ui/SignIn";
import { ssoReady } from "../lib/sso";
import { ADULT_OPTIONS, setAdultPref, useAdult, type Adult } from "../lib/prefs";

export function Settings({ onGallery }: { onGallery?: () => void }) {
  return (
    <div className="cols">
      <NexusAccount />
      <NexusPagesModule />
      <BackgroundModule />
      <GameFolders />
      <InstallPrefs />
      <AdultModule />
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
            <p>{ssoReady() ? "Sign in or paste an API key" : "Paste an API key"} to download, endorse and track</p>
          )}
          {!user && <NexusSignIn onDone={setSt} />}
          <label className="fbox" style={{ flex: "none" }}>
            <input className="finput" type="password" autoComplete="off" spellCheck={false} placeholder={st?.hasKey ? "Replace saved key" : "Personal API key"}
              value={key} onChange={(e) => setKey(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter" && key) void save(); }} aria-label="Nexus API key" />
          </label>
          <div className="actions">
            <Key icon="check" onClick={save} disabled={busy || !key} title="Saved in Windows Credential Manager">Save</Key>
            {!st?.hasKey && <Key variant="ghost" icon="web" onClick={() => nexusPages.url(API_KEY_PAGE).catch((e) => setErr(errorText(e)))} title="Nexus, Site preferences, API keys">Get my key</Key>}
            {st?.hasKey && <Key variant="ghost" icon="trash" onClick={clear} disabled={busy}>Remove key</Key>}
          </div>
          {err && <div className="note bad">{err}</div>}
          {user && !user.isPremium && <PremiumTips />}
        </div>
        {rl && (
          <Lcd label="Requests left" tone={rl.level === "throttled" ? "bad" : rl.level === "low" ? "hot" : "ok"}
            value={rl.remaining ?? "---"} unit={rl.limit ? `/${rl.limit}` : undefined}
            sub={rl.level === "unknown" ? "Shows after the first request" : `This ${rl.window}${rl.level === "low" ? ", slowing down" : rl.level === "throttled" ? ", throttled" : ""}`}
            level={rl.limit && rl.remaining !== null ? { on: Math.ceil((rl.remaining / rl.limit) * 10), of: 10 } : undefined} />
        )}
      </div>
    </Module>
  );
}

/** Free accounts: the line that says what Premium would save before an install with several clicks (ui/Premium.tsx). */
function PremiumTips() {
  const [on, setOn] = useState(true);
  useEffect(() => { settings.get(PREMIUM_TIPS).then((v) => setOn(v !== "0")).catch(() => {}); }, []);
  const set = (v: boolean) => { setOn(v); settings.set(PREMIUM_TIPS, v ? "1" : "0").then(() => window.dispatchEvent(new Event("pm-prefs"))).catch((e) => toast(errorText(e), true)); };
  return (
    <div className="actions">
      <Switch checked={on} onChange={set}>
        <span title="A note before installs that need several Nexus clicks">Show Premium hints</span>
      </Switch>
      <Key size="sm" variant="ghost" icon="web" onClick={getPremium} title="Opens Nexus Premium on the Nexus site">Get Premium</Key>
    </div>
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
  const takeBack = () => handler.register().then((x) => { setH(x); toast("Download links open in Pockymod"); }).catch((e) => toast(errorText(e), true));
  return (
    <Module icon="web" title="Nexus pages" count={where === "browser" ? "in your browser" : "in the app"}>
      <span title={where === "browser" ? "Your sign-in, passwords and extensions" : "A pane beside your mods. Drag its edge to resize"}>
        <Seg<PagesWhere> label="Open Nexus pages" value={where} onChange={pick}
          options={[{ value: "browser", label: "In your browser" }, { value: "pane", label: "In the app" }]} /></span>
      {h?.supported && (h.ours
        ? <p className="dim" style={{ fontSize: ".86em", marginTop: ".4em" }}><Pill tone="ok">Linked</Pill> Download links come here</p>
        : <div className="note warn" style={{ marginTop: ".5em" }}>Download links go to {h.owner ?? "another program"}
            <div className="actions" style={{ marginTop: ".4em" }}><Key size="sm" icon="link" onClick={takeBack}>Use Pockymod</Key></div></div>)}
      {where === "pane" && ext && (
        <>
          <div className="sub-h">Extensions in the pane</div>
          <div className="actions"><Key size="sm" variant="ghost" icon="folder" onClick={() => browserPane.extensionsFolder(true).then(setExt).catch((e) => toast(errorText(e), true))}
            title="Unpacked Chrome extensions. Restart after adding one">Open extensions folder</Key><span className="dim" style={{ fontSize: ".86em" }}>{ext[1] ? `${ext[1]} found` : "None"}</span></div>
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
  const KEYS = { bg: "app.background", up: "app.updates", mods: "mods.background", pre: "mods.prefetch", inst: "mods.autoInstall" } as const;
  useEffect(() => {
    Promise.all(Object.values(KEYS).map((k) => settings.get(k).catch(() => null))).then(([bg, up, mods, pre, inst]) =>
      setV({ bg: bg ?? "1", up: up ?? "auto", mods: mods ?? "0", pre: pre ?? "1", inst: inst ?? "0" }));
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
          <span title="Downloads and update checks keep going. Quit from the tray">Keep running in the tray on close</span>
        </Switch>
        <Switch checked={auto} onChange={(x) => background.setAutostart(x).then(setAuto).catch((e) => toast(errorText(e), true))}>
          Start with Windows
        </Switch>
        <div className="sub-h">App updates</div>
        <Seg label="App updates" value={v.up} onChange={(x) => put("up", x)}
          options={[{ value: "auto", label: "Install when closed" }, { value: "ask", label: "Download only" }, { value: "off", label: "Off" }]} />
        <div className="sub-h">Mod updates</div>
        <Switch checked={v.mods === "1"} onChange={(x) => put("mods", x ? "1" : "0")}>
          <span title="Otherwise on open and on Check now">Check for mod updates every 6 hours</span>
        </Switch>
        {v.mods === "1" && <>
          <Switch checked={v.pre === "1"} onChange={(x) => put("pre", x ? "1" : "0")}>
            <span title="Premium only. Free accounts need a click per file">Download updates ahead</span>
          </Switch>
          <Switch checked={v.inst === "1"} onChange={(x) => put("inst", x ? "1" : "0")}>
            <span title="Only while the window is closed and the game isn't running">Install them when closed</span>
          </Switch>
        </>}
        <div className="actions" style={{ marginTop: ".3em" }}>
          <Key size="sm" icon="refresh" disabled={busy} onClick={() => { setBusy(true); background.check().then(setSt).catch((e) => toast(errorText(e), true)).finally(() => setBusy(false)); }}
            title="Mod updates and app updates">Check now</Key>
          {st?.readyVersion && <Key size="sm" variant="dark" icon="power" onClick={() => background.installUpdate().catch((e) => toast(errorText(e), true))}
            title="Close, install and reopen">Restart into {st.readyVersion}</Key>}
          {st && st.modsReady > 0 && <span className="dim" style={{ fontSize: ".84em" }}>{st.modsReady} {st.modsReady === 1 ? "update" : "updates"} ready</span>}
        </div>
      </div>
    </Module>
  );
}

/** Mods and collections Nexus marks adult, in Browse and Collections. */
function AdultModule() {
  const v = useAdult();
  return (
    <Module icon="eye" title="Adult content" count={v === "hide" ? "hidden" : v === "blur" ? "blurred" : "shown"}>
      <span title={v === "blur" ? "Open a mod and press Show to unblur" : "Mods Nexus marks adult"}><Seg<Adult> label="Adult content" value={v} onChange={(x) => void setAdultPref(x).catch((e) => toast(errorText(e), true))} options={ADULT_OPTIONS} /></span>
    </Module>
  );
}
