// First run: the app finds the game and sets itself up, asking only what only the user can answer.
import { useEffect, useState } from "react";
import { Icon } from "../ui/Icon";
import { Logo } from "../ui/Logo";
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
        <div className="brandmark" style={{ fontSize: "1.3em" }}><Logo /><span>Pockymod</span></div>
        <div className="onb-steps">{STEPS.map((s, i) => <span key={s.id} className={`onb-dot${s.id === step ? " on" : STEPS.findIndex((x) => x.id === step) > i ? " done" : ""}`}>{s.label}</span>)}</div>
        {step === "game" && <GameStep onNext={next} />}
        {step === "account" && <AccountStep onNext={next} />}
        {step === "existing" && <Module icon="folder" title="Mods you already have" band="hot"><ExistingBody compact quiet onDone={() => next()} /><div className="actions" style={{ justifyContent: "flex-end", marginTop: ".4em" }}><Key variant="ghost" onClick={next}>Skip</Key></div></Module>}
        {step === "links" && <LinksStep onNext={next} />}
        {step === "done" && <DoneStep onFinish={finish} />}
        {step !== "done" && <button type="button" className="linkish onb-skip" onClick={finish} title="Set it up later in Settings">Skip setup</button>}
      </div>
    </div>
  );
}

function GameStep({ onNext }: { onNext: () => void }) {
  const [found, setFound] = useState<GameInstall[] | null>(null);
  const [p, setP] = useState<PathsView | null>(null);
  const [lines, setLines] = useState<ScanLine[]>([]);
  // the key whose work is running: a found copy's path, or "pick"
  const [working, setWorking] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    // the stores are read in one quick call; the lines say what it looks at
    const where = ["Steam: libraryfolders.vdf in every Steam library", "GOG Galaxy: the GOG.com registry entries", "Epic Games: the launcher's install manifests"];
    const timers = where.map((w, i) => setTimeout(() => setLines((l) => [...l.map((x) => ({ ...x, done: true })), { text: w }]), i * 220));
    const started = Date.now();
    game.detect().then(async (list) => {
      await new Promise((r) => setTimeout(r, Math.max(0, 700 - (Date.now() - started))));
      if (!live) return;
      setFound(list);
      const cur = await paths.get();
      // one install found and nothing set yet: just use it
      const real = list.filter((g) => g.exists);
      const next = !cur.gameDir && real.length === 1 ? await paths.set({ gameDir: real[0].path }) : cur;
      if (live) setP(next);
    }).catch((e) => { if (!live) return; toast(errorText(e), true); setFound((f) => f ?? []); });
    return () => { live = false; timers.forEach(clearTimeout); };
  }, []);
  const use = async (key: string, dir: () => Promise<string | null>) => {
    setWorking(key);
    try { const d = await dir(); if (d) setP(await paths.set({ gameDir: d })); } catch (e) { toast(errorText(e), true); } finally { setWorking(null); }
  };
  return (
    <Module icon="play" title="Find the game" band="hot" count={p?.gameLooksRight ? "found" : undefined}>
      {!found && <Scanner title="Looking for Cyberpunk 2077" icon="play" lines={lines} />}
      {p?.gameDir && p.gameLooksRight ? (
        <>
          <p>Cyberpunk 2077 is at <code>{p.gameDir}</code>.</p>
          <p className="dim" style={{ fontSize: ".88em" }}>Mods are kept in <code>{p.libraryDir}</code> {p.sameVolume ? <span title="Hard links, no extra space"><Pill tone="ok">Same drive</Pill></span> : <Pill tone="soon">another drive</Pill>}</p>
          {p.inProgramFiles && <div className="note warn">Game is under Program Files, where Windows can block mods</div>}
        </>
      ) : found && (
        <div className="stack" style={{ gap: ".4em" }}>
          {p?.gameDir && <div className="note bad">No bin\x64 in <code>{p.gameDir}</code>. Pick the folder with bin, archive and r6</div>}
          <p>{found.length ? "Pick your copy" : "Game not found. Choose its folder (with bin, archive and r6)"}</p>
          {found.map((g) => (
            <div key={g.path} className="actions" style={{ flexWrap: "nowrap" }}>
              <Pill tone={g.exists ? "ok" : undefined}>{g.store}</Pill>
              <code style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{g.path}</code>
              <Key size="sm" disabled={!g.exists || working !== null} busy={working === g.path} title={g.exists ? undefined : "That folder is gone"}
                onClick={() => use(g.path, async () => g.path)}>Use</Key>
            </div>
          ))}
        </div>
      )}
      <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".8em" }}>
        <Key variant="ghost" icon="folder" disabled={working !== null} busy={working === "pick"} onClick={() => use("pick", () => pickFolder("Cyberpunk 2077 folder"))}>Choose a folder</Key>
        <Key variant="dark" icon="chevron" disabled={!p?.gameLooksRight} onClick={onNext} title={p?.gameLooksRight ? undefined : "Find or choose the game folder first"}>Next</Key>
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
    if (!key || busy) return;
    setBusy(true);
    try { const s = await nexus.setKey(key); setName(s.user?.name ?? "connected"); setPremium(!!s.user?.isPremium); setKey(""); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
  };
  return (
    <Module icon="key" title="Connect Nexus Mods" band="hot" count="optional">
      {name ? (
        <p>Connected as <b className="hi">{name}</b> {premium ? <Pill tone="ok">Premium</Pill> : <Pill>Free</Pill>}</p>
      ) : (
        <>
          {ssoReady() ? <>
            <p>Sign in to download. Browsing works without it</p>
            <NexusSignIn primary onDone={(s) => { setName(s.user?.name ?? "connected"); setPremium(!!s.user?.isPremium); }} />
            <p className="dim" style={{ fontSize: ".84em", marginTop: ".6em" }}>Or paste an API key</p>
          </> : <p>Paste your Nexus API key to download. Browsing works without it</p>}
          <div className="actions" style={{ flexWrap: "nowrap", marginTop: ".5em" }}>
            <label className="fbox"><input className="finput" type="password" autoComplete="off" value={key} onChange={(e) => setKey(e.target.value)} placeholder="Personal API key" aria-label="Nexus API key" onKeyDown={(e) => { if (e.key === "Enter") void save(); }} /></label>
            <Key icon="check" disabled={!key} busy={busy} onClick={save} title={key ? "Saved in Windows Credential Manager" : "Paste a key first"}>Connect</Key>
          </div>
          <button type="button" className="linkish" style={{ marginTop: ".4em", fontSize: ".86em" }}
            title="nexusmods.com, your avatar, Site preferences, API keys"
            onClick={() => nexusPages.url(API_KEY_PAGE).catch((e) => toast(errorText(e), true))}>Get my key</button>
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
  const [lines, setLines] = useState<ScanLine[]>([{ text: "Checking who opens nxm:// links" }]);
  const say = (text: string) => setLines((l) => [...l.map((x) => ({ ...x, done: true })), { text }]);
  const [linking, setLinking] = useState(false);
  useEffect(() => {
    let live = true;
    const started = Date.now();
    const settle = async (r: HandlerStatus) => { await new Promise((ok) => setTimeout(ok, Math.max(0, 900 - (Date.now() - started)))); if (!live) return; setLines((l) => l.map((x) => ({ ...x, done: true }))); setH(r); };
    handler.status().then(async (s) => {
      if (!s.supported) { say("Not Windows, the system handles links"); return settle(s); }
      if (s.ours) { say("Already Pockymod"); return settle(s); }
      // nobody handles nxm:// links: take them without asking; another manager does: ask
      if (!s.owner) {
        say("No handler yet, registering Pockymod");
        const r = await handler.register().catch(() => s);
        return settle(r);
      }
      say(`${s.owner} has them`);
      return settle(s);
    }).catch(() => settle({ supported: false, ours: false, owner: null, command: null }));
    return () => { live = false; };
  }, []);
  const settled = !!h && (!h.supported || h.ours);
  useEffect(() => { if (!settled) return; const t = setTimeout(onNext, 2600); return () => clearTimeout(t); }, [settled, onNext]);
  if (!h) return <Module icon="link" title="Download links"><Scanner title="Checking download links" icon="link" lines={lines} /></Module>;
  if (settled) return (
    <Module icon="link" title="Download links" count="done">
      <p><Icon name="check" size={9} /> {h.supported ? "Mod Manager Download on Nexus opens in Pockymod" : "Your system handles browser links"}</p>
      <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".8em" }}><Key variant="dark" icon="chevron" onClick={onNext}>Next</Key></div>
    </Module>
  );
  const register = () => {
    setLinking(true);
    handler.register().then(() => { toast("Links open in Pockymod"); onNext(); }).catch((e) => { toast(errorText(e), true); setLinking(false); });
  };
  // another manager has the links: ask. No owner here means registering by itself failed: offer it again
  return (
    <Module icon="link" title="Download links" band="hot">
      <p>{h.owner ? `${h.owner} opens Mod Manager Download links from your browser` : "Couldn't set Pockymod to open Mod Manager Download links"}</p>
      <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".8em" }}>
        {h.owner ? <Key variant="ghost" disabled={linking} onClick={() => { void settings.set("links.other", "1").catch(() => {}); onNext(); }}>Keep {h.owner}</Key>
          : <Key variant="ghost" disabled={linking} onClick={onNext} title="Set it up later in Settings, Game and mod folders">Later</Key>}
        <Key variant="dark" icon="link" busy={linking} onClick={register}>{h.owner ? "Use Pockymod" : "Try again"}</Key>
      </div>
    </Module>
  );
}

function DoneStep({ onFinish }: { onFinish: () => void }) {
  const [problems, setProblems] = useState<number | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => { game.health().then((r) => setProblems(r.problems)).catch(() => setFailed(true)); }, []);
  return (
    <Module icon="check" title="Ready" band="hot">
      <div className="lcds"><Lcd size="sm" label="Problems" value={problems === null ? "--" : String(problems).padStart(2, "0")} tone={problems ? "bad" : "ok"}
        sub={problems === 0 ? "All good" : problems || failed ? "See Health" : "Checking"} /></div>
      <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".8em" }}><Key variant="dark" icon="play" onClick={onFinish}>Start</Key></div>
    </Module>
  );
}
