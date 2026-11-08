import { useEffect, useState } from "react";
import { Key, Lcd, Module, Pill, Seg } from "../ui/pocket";
import { background, browserPane, errorText, handler, nexus, nexusPages, pagesWhere, settings, updates, type BackgroundState, type HandlerStatus, type NexusStatus, type PagesWhere } from "../lib/api";
import { Switch } from "../ui/pocket";
import { toast } from "../ui/toast";
import { API_KEY_PAGE } from "./Onboarding";
import { Appearance } from "./Appearance";
import { GameFolders } from "./GameFolders";
import { About, Experiments } from "./About";
import { InstallPrefs } from "./InstallPrefs";
import { PREMIUM_TIPS, getPremium } from "../ui/Premium";
import { NexusSignIn } from "../ui/SignIn";
import { ADULT_OPTIONS, setAdultPref, setAiPref, useAdult, useAiHide, type Adult } from "../lib/prefs";
import { AI_OPTIONS, type AiHide } from "../lib/ai";
import type { PaneSignIn } from "../lib/api";

export function Settings({ onGallery }: { onGallery?: () => void }) {
  return (
    <div className="cols">
      <NexusAccount />
      <NexusPagesModule />
      <BackgroundModule />
      <GameFolders />
      <InstallPrefs />
      <AdultModule />
      <AiModule />
      <Appearance />
      <Experiments />
      <About onGallery={onGallery} />
    </div>
  );
}

function NexusAccount() {
  const [st, setSt] = useState<NexusStatus | null>(null);
  const [key, setKey] = useState("");
  // the key whose work is running
  const [busy, setBusy] = useState<"save" | "clear" | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  // with sign-in on offer, the key field is an advanced option (the creator tools' Upload API still uses a key)
  const [showKey, setShowKey] = useState(false);

  useEffect(() => { nexus.status().then(setSt).catch((e) => setErr(errorText(e))).finally(() => setLoaded(true)); }, []);

  const save = async () => {
    if (!key || busy) return;
    setBusy("save"); setErr(null);
    try { setSt(await nexus.setKey(key)); setKey(""); toast("Key saved"); } catch (e) { setErr(errorText(e)); } finally { setBusy(null); }
  };
  const clear = async () => {
    setBusy("clear"); setErr(null);
    try { const wasSignedIn = st?.signedInWith === "nexus"; setSt(await nexus.clearKey()); setShowKey(false); toast(wasSignedIn ? "Signed out" : "Key removed"); } catch (e) { setErr(errorText(e)); } finally { setBusy(null); }
  };
  const user = st?.user;
  const signedIn = st?.signedInWith === "nexus";
  const keyOpen = !st?.signInAvailable || st.signedInWith === "key" || showKey;
  const rl = st?.rateLimit;

  return (
    <Module icon="key" title="Nexus account" count={user ? user.name : st?.hasKey ? "key saved" : loaded ? "not connected" : undefined}>
      <div className="grid2" style={{ alignItems: "start" }}>
        <div className="stack" style={{ gap: ".7em" }}>
          {!loaded ? (
            <span className="skel skel-line" style={{ width: "60%" }} aria-hidden="true" />
          ) : user ? (
            <div className="actions">
              <b className="hi">{user.name}</b>
              {user.isPremium ? <Pill tone="ok">Premium</Pill> : <Pill>Free account</Pill>}
              {user.isSupporter && <Pill>Supporter</Pill>}
            </div>
          ) : (
            <p>{st?.signInAvailable ? "Sign in" : "Paste an API key"} to download, endorse and track</p>
          )}
          {loaded && !st?.hasKey && <NexusSignIn status={st} primary onDone={setSt} />}
          {signedIn && (
            <div className="actions">
              <Key variant="ghost" icon="power" onClick={clear} disabled={!!busy} busy={busy === "clear"} title="Forget this sign-in, and any saved API key, on this PC">Sign out</Key>
            </div>
          )}
          {loaded && st?.signInAvailable && st.signedInWith !== "key" && (
            <div className="actions">
              <Key size="sm" variant="ghost" icon="chevron" className={keyOpen ? "rot-key" : ""} onClick={() => setShowKey(!showKey)} aria-expanded={keyOpen}
                title={signedIn ? "Only the creator tools use a key, to publish your mods" : "A personal key from your Nexus settings, if you'd rather not sign in"}>
                {signedIn ? (st.keySaved ? "API key for publishing, saved" : "API key for publishing") : "Use an API key instead"}</Key>
            </div>
          )}
          {loaded && keyOpen && <>
            <label className="fbox" style={{ flex: "none" }}>
              <input className="finput" type="password" autoComplete="off" spellCheck={false} placeholder={st?.keySaved ? "Replace saved key" : "Personal API key"}
                value={key} onChange={(e) => setKey(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter" && key) void save(); }} aria-label="Nexus API key" />
            </label>
            <div className="actions">
              <Key icon="check" onClick={save} disabled={!!busy || !key} busy={busy === "save"} title={key ? "Saved in Windows Credential Manager" : "Paste a key first"}>Save</Key>
              {st?.signedInWith !== "key" && <Key variant="ghost" icon="web" onClick={() => nexusPages.url(API_KEY_PAGE).catch((e) => setErr(errorText(e)))} title="Nexus, Site preferences, API keys">Get my key</Key>}
              {st?.signedInWith === "key" && <Key variant="ghost" icon="trash" onClick={clear} disabled={!!busy} busy={busy === "clear"} title="Forget the key saved in Windows Credential Manager">Remove key</Key>}
            </div>
          </>}
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
      <Key size="sm" variant="ghost" icon="web" onClick={() => getPremium("settings")} title="Opens Nexus Premium on the Nexus site">Get Premium</Key>
    </div>
  );
}

/** Where Nexus pages open: the user's own browser (the default) or the app's browser pane. */
function NexusPagesModule() {
  const [where, setWhere] = useState<PagesWhere | null>(null);
  const [h, setH] = useState<HandlerStatus | null>(null);
  const [ext, setExt] = useState<[string, number] | null>(null);
  const [linking, setLinking] = useState(false);
  useEffect(() => {
    pagesWhere.get().then(setWhere).catch(() => setWhere("browser"));
    handler.status().then(setH).catch(() => {});
    browserPane.extensionsFolder(false).then(setExt).catch(() => {});
  }, []);
  if (!where) return null;
  const pick = (w: PagesWhere) => { setWhere(w); void pagesWhere.set(w).catch((e) => toast(errorText(e), true)); };
  const takeBack = () => {
    setLinking(true);
    handler.register().then((x) => { setH(x); toast("Download links open in Pockymod"); }).catch((e) => toast(errorText(e), true)).finally(() => setLinking(false));
  };
  return (
    <Module icon="web" title="Nexus pages" count={where === "browser" ? "in your browser" : "in the app"}>
      <span title={where === "browser" ? "Your sign-in, passwords and extensions" : "A pane beside your mods. Drag its edge to resize"}>
        <Seg<PagesWhere> label="Open Nexus pages" value={where} onChange={pick}
          options={[{ value: "browser", label: "In your browser" }, { value: "pane", label: "In the app" }]} /></span>
      {h?.supported && (h.ours
        ? <p className="dim" style={{ fontSize: ".86em", marginTop: ".4em" }}><Pill tone="ok">Linked</Pill> Download links come here</p>
        : <div className="note warn" style={{ marginTop: ".5em" }}>Download links go to {h.owner ?? "another program"}
            <div className="actions" style={{ marginTop: ".4em" }}><Key size="sm" icon="link" busy={linking} onClick={takeBack}>Use Pockymod</Key></div></div>)}
      {where === "pane" && <PaneSignInRow />}
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

/** The app's browser's own Nexus sign-in, and a way to sign out (which clears what the pane saved). */
function PaneSignInRow() {
  const [s, setS] = useState<PaneSignIn | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    // an event that lands first is newer than the first reading
    browserPane.signIn().then((x) => setS((cur) => cur ?? x)).catch(() => {});
    // signing in or out on a Nexus page shows here straight away
    let off: (() => void) | undefined, gone = false;
    browserPane.onSignIn(setS).then((u) => { if (gone) u(); else off = u; });
    return () => { gone = true; off?.(); };
  }, []);
  if (!s) return null;
  const out = () => {
    setBusy(true);
    browserPane.signOut().then((x) => { setS(x); toast("Signed out of Nexus in the app"); })
      .catch((e) => toast(errorText(e), true)).finally(() => setBusy(false));
  };
  return (
    <>
      <div className="sub-h">Nexus sign-in in the app</div>
      {s.signedIn ? (
        <div className="actions"><Pill tone="ok">Signed in</Pill>
          <Key size="sm" variant="ghost" icon="trash" busy={busy} onClick={out} title="Signs the app's browser out of Nexus and forgets the passwords it saved">Sign out</Key></div>
      ) : (
        <p className="dim" style={{ fontSize: ".86em" }}>Sign in on any Nexus page in the pane. It offers to save your password, so you only type it once.</p>
      )}
    </>
  );
}

/** Running in the background and quiet updates: each one a choice, on by default except starting with Windows and
 *  installing mod updates by themselves. */
function BackgroundModule() {
  const [v, setV] = useState<Record<string, string> | null>(null);
  const [auto, setAuto] = useState(false);
  const [st, setSt] = useState<BackgroundState | null>(null);
  const [busy, setBusy] = useState<"check" | "restart" | null>(null);
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
  // a check that finds nothing still says so
  const check = async () => {
    setBusy("check");
    try {
      const s = await background.check();
      setSt(s);
      const n = (await updates.last().catch(() => [])).length;
      toast(n ? `${n} mod ${n === 1 ? "update" : "updates"}, listed in Library` : "No mod updates");
    } catch (e) { toast(errorText(e), true); } finally { setBusy(null); }
  };
  const restart = () => { setBusy("restart"); background.installUpdate().catch((e) => { toast(errorText(e), true); setBusy(null); }); };
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
          <Key size="sm" icon="refresh" disabled={!!busy} busy={busy === "check"} onClick={check}
            title="Mod updates and app updates">Check now</Key>
          {st?.readyVersion && <Key size="sm" variant="dark" icon="power" disabled={!!busy} busy={busy === "restart"} onClick={restart}
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

/** Mods Nexus tags as made with AI: shown with a label (the default), or left out of Browse. */
function AiModule() {
  const v = useAiHide();
  return (
    <Module icon="eye" title="AI content" count={v === "show" ? "shown" : v === "generated" ? "AI-generated hidden" : "all AI hidden"}>
      <span title="Hide AI-generated leaves out mods Nexus tags AI-Generated Content. Hide all AI also leaves out AI Media and AI Assisted"><Seg<AiHide> label="AI-tagged mods" value={v} onChange={(x) => void setAiPref(x).catch((e) => toast(errorText(e), true))} options={AI_OPTIONS} /></span>
      <p className="dim" style={{ fontSize: ".84em", marginTop: ".5em" }}>Browse labels mods with Nexus's AI tags. Authors tag their own mods, so one with no tag can't be told apart. If you see one that should have a tag, report it on its Nexus page.</p>
    </Module>
  );
}
