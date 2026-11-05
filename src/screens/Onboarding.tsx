// First run: the app finds the game and sets itself up, asking only what only the user can answer.
import { useEffect, useState } from "react";
import { Icon } from "../ui/Icon";
import { Key, Lcd, Module, Pill } from "../ui/pocket";
import { errorText, game, handler, nexus, nexusPages, paths, pickFolder, settings, type GameInstall, type HandlerStatus, type PathsView } from "../lib/api";
import { ExistingBody } from "./Existing";
import { Scanner, type ScanLine } from "../ui/Scanner";
import { toast } from "../ui/toast";
import { NexusSignIn } from "../ui/SignIn";
import { ssoReady } from "../lib/sso";

/** Nexus's personal API key page (its account settings). */
export const API_KEY_PAGE = "https://next.nexusmods.com/settings/api-keys";

type Step = "game" | "account" | "existing" | "links" | "done";
const STEPS: Array<{ id: Step; label: string }> = [
  { id: "game", label: "Game" }, { id: "account", label: "Nexus" }, { id: "existing", label: "Your mods" }, { id: "links", label: "Links" }, { id: "done", label: "Ready" },
];

export function Onboarding({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState<Step>("game");
  const next = () => setStep(STEPS[Math.min(STEPS.length - 1, STEPS.findIndex((s) => s.id === step) + 1)].id);
  const finish = () => { void settings.set("onboarded", "1"); onDone(); };
  return (
    <div className="ov full onboard">
      <div className="onb">
        <div className="brandmark" style={{ fontSize: "1.3em" }}><span className="logo">P</span><span>Pockymod</span></div>
        <div className="onb-steps">{STEPS.map((s, i) => <span key={s.id} className={`onb-dot${s.id === step ? " on" : STEPS.findIndex((x) => x.id === step) > i ? " done" : ""}`}>{s.label}</span>)}</div>
        {step === "game" && <GameStep onNext={next} />}
        {step === "account" && <AccountStep onNext={next} />}
        {step === "existing" && <Module icon="folder" title="Mods you already have" band="hot"><ExistingBody compact quiet onDone={() => next()} /><div className="actions" style={{ justifyContent: "flex-end", marginTop: ".4em" }}><Key variant="ghost" onClick={next}>Skip</Key></div></Module>}
        {step === "links" && <LinksStep onNext={next} />}
        {step === "done" && <DoneStep onFinish={finish} />}
        {step !== "done" && <button type="button" className="linkish onb-skip" onClick={finish}>Skip setup, I will do it in Settings</button>}
      </div>
    </div>
  );
}

function GameStep({ onNext }: { onNext: () => void }) {
  const [found, setFound] = useState<GameInstall[] | null>(null);
  const [p, setP] = useState<PathsView | null>(null);
  const [lines, setLines] = useState<ScanLine[]>([]);
  useEffect(() => {
    // the stores are read in one quick call; the lines say what it looks at
    const where = ["Steam: libraryfolders.vdf in every Steam library", "GOG Galaxy: the GOG.com registry entries", "Epic Games: the launcher's install manifests"];
    where.forEach((w, i) => setTimeout(() => setLines((l) => [...l.map((x) => ({ ...x, done: true })), { text: w }]), i * 220));
    const started = Date.now();
    game.detect().then(async (list) => {
      await new Promise((r) => setTimeout(r, Math.max(0, 700 - (Date.now() - started))));
      setFound(list);
      const cur = await paths.get();
      // one install found and nothing set yet: just use it
      const real = list.filter((g) => g.exists);
      if (!cur.gameDir && real.length === 1) setP(await paths.set({ gameDir: real[0].path }));
      else setP(cur);
    }).catch((e) => { toast(errorText(e), true); setFound([]); });
  }, []);
  const use = async (dir: string) => setP(await paths.set({ gameDir: dir }));
  return (
    <Module icon="play" title="Find the game" band="hot" count={p?.gameLooksRight ? "found" : undefined}>
      {!found && <Scanner title="Looking for Cyberpunk 2077" icon="play" lines={lines} />}
      {p?.gameDir && p.gameLooksRight ? (
        <>
          <p>Cyberpunk 2077 is at <code>{p.gameDir}</code>.</p>
          <p className="dim" style={{ fontSize: ".88em" }}>Mods are kept in <code>{p.libraryDir}</code> {p.sameVolume ? <Pill tone="ok">same drive: instant, no extra space</Pill> : <Pill tone="soon">another drive</Pill>}</p>
          {p.inProgramFiles && <div className="note warn">The game is under Program Files, where Windows can block mods. If an install fails, Settings shows the fix.</div>}
        </>
      ) : found && (
        <div className="stack" style={{ gap: ".4em" }}>
          <p>{found.length ? "Pick your copy of the game:" : "The game was not found on Steam, GOG or Epic. Choose its folder (the one with bin, archive and r6 inside)."}</p>
          {found.map((g) => (
            <div key={g.path} className="actions" style={{ flexWrap: "nowrap" }}>
              <Pill tone={g.exists ? "ok" : undefined}>{g.store}</Pill>
              <code style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{g.path}</code>
              <Key size="sm" disabled={!g.exists} onClick={() => use(g.path)}>Use</Key>
            </div>
          ))}
        </div>
      )}
      <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".8em" }}>
        <Key variant="ghost" icon="folder" onClick={async () => { const d = await pickFolder("The Cyberpunk 2077 folder"); if (d) await use(d); }}>Choose a folder</Key>
        <Key variant="dark" icon="chevron" disabled={!p?.gameLooksRight} onClick={onNext}>Next</Key>
      </div>
    </Module>
  );
}

function AccountStep({ onNext }: { onNext: () => void }) {
  const [key, setKey] = useState("");
  const [name, setName] = useState<string | null>(null);
  const [premium, setPremium] = useState(false);
  const [busy, setBusy] = useState(false);
  useEffect(() => { nexus.status().then((s) => { if (s.user) { setName(s.user.name); setPremium(s.user.isPremium); } }).catch(() => {}); }, []);
  const save = async () => {
    setBusy(true);
    try { const s = await nexus.setKey(key); setName(s.user?.name ?? "connected"); setPremium(!!s.user?.isPremium); setKey(""); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
  };
  return (
    <Module icon="key" title="Connect Nexus Mods" band="hot" count="optional">
      {name ? (
        <p>Connected as <b className="hi">{name}</b> {premium ? <Pill tone="ok">Premium: installs run by themselves</Pill> : <Pill>Free: one click per file on Nexus</Pill>}</p>
      ) : (
        <>
          {ssoReady() ? <>
            <p>Browsing needs nothing. To download, sign in with your Nexus Mods account. The key it gives Pockymod is kept in Windows Credential Manager.</p>
            <NexusSignIn primary onDone={(s) => { setName(s.user?.name ?? "connected"); setPremium(!!s.user?.isPremium); }} />
            <p className="dim" style={{ fontSize: ".84em", marginTop: ".6em" }}>Or paste a personal API key:</p>
          </> : <p>Browsing needs nothing. To download, paste your personal API key from Nexus Mods (Site preferences, API keys). It is kept in Windows Credential Manager.</p>}
          <div className="actions" style={{ flexWrap: "nowrap", marginTop: ".5em" }}>
            <label className="fbox"><input className="finput" type="password" autoComplete="off" value={key} onChange={(e) => setKey(e.target.value)} placeholder="Personal API key" aria-label="Nexus API key" onKeyDown={(e) => { if (e.key === "Enter" && key) void save(); }} /></label>
            <Key icon="check" disabled={!key || busy} onClick={save}>Connect</Key>
          </div>
          <button type="button" className="linkish" style={{ marginTop: ".4em", fontSize: ".86em" }}
            onClick={() => nexusPages.url(API_KEY_PAGE).then(() => toast("The API key page opens in a Nexus window. Sign in there if asked, then copy the Personal API key.")).catch((e) => toast(errorText(e), true))}>Open the API key page in the app</button>
          <p className="dim" style={{ fontSize: ".8em", marginTop: ".2em" }}>Or in any browser: nexusmods.com, your avatar, Site preferences, API keys.</p>
        </>
      )}
      <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".8em" }}>
        <Key variant={name ? "dark" : "ghost"} icon="chevron" onClick={onNext}>{name ? "Next" : "Later"}</Key>
      </div>
    </Module>
  );
}

function LinksStep({ onNext }: { onNext: () => void }) {
  const [h, setH] = useState<HandlerStatus | null>(null);
  const [lines, setLines] = useState<ScanLine[]>([{ text: "Reading which app opens nxm:// links (HKEY_CURRENT_USER\\Software\\Classes\\nxm)" }]);
  const [took, setTook] = useState(false);
  const say = (text: string) => setLines((l) => [...l.map((x) => ({ ...x, done: true })), { text }]);
  useEffect(() => {
    const started = Date.now();
    const settle = async (r: HandlerStatus) => { await new Promise((ok) => setTimeout(ok, Math.max(0, 900 - (Date.now() - started)))); setLines((l) => l.map((x) => ({ ...x, done: true }))); setH(r); };
    handler.status().then(async (s) => {
      if (!s.supported) { say("Not on Windows: links are handled by the system"); return settle(s); }
      if (s.ours) { say("Pockymod already opens them"); return settle(s); }
      // nobody handles nxm:// links: take them without asking; another manager does: ask
      if (!s.owner) {
        say("No app opens them yet: registering Pockymod");
        const r = await handler.register().catch(() => s);
        setTook(r.ours);
        return settle(r);
      }
      say(`${s.owner} opens them now`);
      return settle(s);
    }).catch(() => settle({ supported: false, ours: false, owner: null, command: null }));
  }, []);
  const settled = !!h && (!h.supported || h.ours);
  useEffect(() => { if (!settled) return; const t = setTimeout(onNext, 2600); return () => clearTimeout(t); }, [settled, onNext]);
  if (!h) return <Module icon="link" title="Download links"><Scanner title="Checking browser download links" icon="link" lines={lines} /></Module>;
  if (settled) return (
    <Module icon="link" title="Download links" count="done">
      <p><Icon name="check" size={9} /> {h.supported ? (took ? "Mod Manager Download buttons on nexusmods.com now open in Pockymod." : "Mod Manager Download buttons on nexusmods.com open in Pockymod.") : "Browser links are handled by your system."}</p>
      <p className="dim" style={{ fontSize: ".86em", marginTop: ".3em" }}>You can test or change this any time in Settings, Download links.</p>
      <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".8em" }}><Key variant="dark" icon="chevron" onClick={onNext}>Next</Key></div>
    </Module>
  );
  return (
    <Module icon="link" title="Download links from your browser" band="hot">
      <p>{h.owner} opens "Mod Manager Download" links clicked in a web browser. Pages opened inside Pockymod always come here, whatever you choose.</p>
      <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".8em" }}>
        <Key variant="ghost" onClick={() => { void settings.set("links.other", "1"); onNext(); }}>Keep {h.owner}</Key>
        <Key variant="dark" icon="link" onClick={() => handler.register().then(() => { toast("Browser links now open in Pockymod."); onNext(); }).catch((e) => toast(errorText(e), true))}>Use Pockymod</Key>
      </div>
    </Module>
  );
}

function DoneStep({ onFinish }: { onFinish: () => void }) {
  const [problems, setProblems] = useState<number | null>(null);
  useEffect(() => { game.health().then((r) => setProblems(r.problems)).catch(() => setProblems(0)); }, []);
  return (
    <Module icon="check" title="Ready" band="hot">
      <div className="lcds"><Lcd size="sm" label="Problems" value={problems === null ? "--" : String(problems).padStart(2, "0")} tone={problems ? "bad" : "ok"} sub={problems ? "Health shows the fixes" : "Nothing broken"} /></div>
      <p style={{ marginTop: ".6em" }}><Icon name="browse" size={9} /> Browse to find mods. Install brings what a mod needs along with it.</p>
      <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".8em" }}><Key variant="dark" icon="play" onClick={onFinish}>Start</Key></div>
    </Module>
  );
}
