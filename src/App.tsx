import { useEffect, useState } from "react";
import { Icon } from "./ui/Icon";
import type { IconName } from "./ui/icons";
import { Empty, Key, ProgramKey, type Led } from "./ui/pocket";
import { Gallery } from "./screens/Gallery";
import { Settings } from "./screens/Settings";
import { Browse } from "./screens/Browse";
import { Downloads } from "./screens/Downloads";
import { Library } from "./screens/Library";
import { InstallDialog } from "./screens/InstallDialog";
import { Health } from "./screens/Health";
import { LoadOrder } from "./screens/LoadOrder";
import { Collections } from "./screens/Collections";
import { ExistingDialog } from "./screens/Existing";
import { ProfileSwitch } from "./ui/ProfileSwitch";
import { Onboarding } from "./screens/Onboarding";
import { LaunchDeck } from "./ui/LaunchDeck";
import { HistoryMenu } from "./ui/HistoryMenu";
import { installUndoKeys } from "./lib/history";
import { FomodDialog } from "./screens/FomodDialog";
import { Create } from "./screens/Create";
import { Toasts, toast } from "./ui/toast";
import { installPrefs } from "./lib/prefs";
import { errorText, game, nexus, nexusPages, onChoicesNeeded, onToast, queue, settings, type ChoicesNeeded, updates as updatesApi, type QueueSnapshot, type Update } from "./lib/api";
import { applyTheme, getThemeChoice, setThemeChoice, watchSystemTheme } from "./lib/theme";
import { windowAction } from "./lib/tauri";
import { RateLcd } from "./ui/RateLcd";

export type View = "browse" | "library" | "collections" | "downloads" | "loadorder" | "health" | "settings" | "create" | "gallery";

const VIEWS: Array<{ id: View; label: string; icon: IconName }> = [
  { id: "browse", label: "Browse", icon: "browse" },
  { id: "library", label: "Library", icon: "library" },
  { id: "collections", label: "Collections", icon: "collections" },
  { id: "downloads", label: "Downloads", icon: "downloads" },
  { id: "loadorder", label: "Load order", icon: "loadorder" },
  { id: "health", label: "Health", icon: "health" },
  { id: "create", label: "Create", icon: "share" },
  { id: "settings", label: "Settings", icon: "settings" },
];

export function App() {
  const [view, setView] = useState<View>("browse");
  const [scheme, setScheme] = useState<"cream" | "night">(() => applyTheme());
  const [snap, setSnap] = useState<QueueSnapshot | null>(null);
  const [premium, setPremium] = useState(false);
  const [installing, setInstalling] = useState<{ modId: number; fileId: number; name: string } | null>(null);
  const [updates, setUpdates] = useState<Update[]>([]);
  const [healthIssues, setHealthIssues] = useState(0);
  const [existingOpen, setExistingOpen] = useState(false);
  const [onboard, setOnboard] = useState(false);
  const [choices, setChoices] = useState<ChoicesNeeded | null>(null);
  useEffect(() => { let off: (() => void) | undefined; onChoicesNeeded(setChoices).then((u) => { off = u; }); return () => off?.(); }, []);
  useEffect(() => { settings.get("onboarded").then((v) => setOnboard(v !== "1")).catch(() => {}); }, []);

  useEffect(() => watchSystemTheme(() => setScheme(applyTheme())), []);
  useEffect(() => installUndoKeys(), []);
  useEffect(() => {
    const offs: Array<() => void> = [];
    queue.get().then(setSnap).catch(() => {});
    queue.onChange(setSnap).then((u) => offs.push(u));
    onToast((m) => toast(m)).then((u) => offs.push(u));
    nexus.status().then((s) => setPremium(!!s.user?.isPremium)).catch(() => {});
    // update badges: the last result now, a fresh check shortly after start and every six hours
    updatesApi.last().then(setUpdates).catch(() => {});
    updatesApi.onChange(setUpdates).then((u) => offs.push(u));
    const check = () => updatesApi.check().catch(() => {});
    const t1 = setTimeout(check, 4000);
    const t2 = setInterval(check, 6 * 3600 * 1000);
    game.health().then((h) => setHealthIssues(h.problems)).catch(() => {});
    return () => { offs.forEach((f) => f()); clearTimeout(t1); clearInterval(t2); };
  }, []);

  const s = snap?.summary;
  const current = snap?.items.find((i) => i.state.state === "downloading");
  const progress = current && current.state.state === "downloading" && current.state.total ? current.state.received / current.state.total : undefined;

  // Install: straight to the queue when the user turned the question off, else the plan dialog
  const startInstall = async (modId: number, fileId: number, name: string) => {
    const p = await installPrefs().catch(() => ({ confirm: true, requirements: "always" as const }));
    if (p.confirm || p.requirements === "ask") { setInstalling({ modId, fileId, name }); return; }
    try {
      const snap = await queue.add(modId, fileId, p.requirements === "always");
      const n = snap.items.filter((i) => i.state.state === "waiting" || i.state.state === "needsClick").length;
      toast(`${name} is queued${n > 1 ? ` with ${n - 1} required mods` : ""}.`);
    } catch (e) { toast(errorText(e), true); }
  };

  const toggleScheme = () => {
    const next = scheme === "night" ? "cream" : "night";
    setThemeChoice(next);
    setScheme(next);
  };

  const led = (id: View): Led | undefined =>
    id === "downloads" ? (s?.running ? "run" : s?.waitingOnUser ? "wait" : "off") : id === "health" && healthIssues > 0 ? "bad" : undefined;

  const here = [...VIEWS, { id: "gallery" as View, label: "Gallery", icon: "gallery" as IconName }].find((v) => v.id === view)!;

  return (
    <div className="screen shell">
      <header className="mtop" data-tauri-drag-region onDoubleClick={(e) => { if (e.target === e.currentTarget) void windowAction("toggleMaximize"); }}>
        <span className="brandmark" data-tauri-drag-region><span className="logo">P</span><span>Pocket Mods</span></span>
        <span className="crumb" data-tauri-drag-region><span className="v">{here.label}</span><span className="ctx">Cyberpunk 2077</span></span>
        <span className="sp" data-tauri-drag-region />
        <HistoryMenu onRemoved={() => { setView("library"); setTimeout(() => document.getElementById("recently-removed")?.scrollIntoView({ behavior: "smooth" }), 120); }} />
        <ProfileSwitch onManage={() => setView("loadorder")} />
        <Key variant="ghost" size="sm" icon={scheme === "night" ? "sun" : "moon"} onClick={toggleScheme}
          aria-label={scheme === "night" ? "Switch to Cream" : "Switch to Night"} title={`Theme: ${getThemeChoice()}`} />
        <span className="winkeys">
          <button type="button" className="wk" aria-label="Minimise" onClick={() => void windowAction("minimize")}><Icon name="minus" size={9} /></button>
          <button type="button" className="wk" aria-label="Maximise" onClick={() => void windowAction("toggleMaximize")}><Icon name="max" size={9} /></button>
          <button type="button" className="wk wk-x" aria-label="Close" onClick={() => void windowAction("close")}><Icon name="x" size={9} /></button>
        </span>
      </header>

      <nav className="side" aria-label="Views">
        <div className="nkeys rail">
          {VIEWS.map((v) => (
            <ProgramKey key={v.id} icon={v.icon} label={v.label} on={view === v.id} onClick={() => setView(v.id)}
              led={led(v.id)}
              count={v.id === "library" ? updates.length : v.id === "downloads" ? s?.left : undefined}
              countHot={v.id === "library"}
              progress={v.id === "downloads" && s?.running ? progress ?? 0 : undefined} />
          ))}
        </div>
        <div className="foot">
          <LaunchDeck onHealth={() => setView("health")} />
          <RateLcd />
          <ProgramKey icon="gallery" label="Gallery" on={view === "gallery"} onClick={() => setView("gallery")} />
        </div>
      </nav>

      <main className="view">
        {/* Browse stays mounted so its search and filters survive a trip to another view */}
        <div style={{ display: view === "browse" ? "contents" : "none" }}>
          <Browse onInstall={(d, fileId) => void startInstall(d.detail.modId, fileId, d.detail.name)}
            onOpenPage={(modId) => nexusPages.mod(modId).catch((e) => toast(errorText(e), true))} />
        </div>
        {view === "gallery" ? <Gallery />
          : view === "settings" ? <Settings scheme={scheme} onScheme={setScheme} />
          : view === "downloads" ? <Downloads snap={snap} />
          : view === "library" ? <Library updates={updates} onExisting={() => setExistingOpen(true)} />
          : view === "health" ? <Health onNavigate={setView} onIssues={setHealthIssues} />
          : view === "loadorder" ? <LoadOrder />
          : view === "create" ? <Create />
          : view === "collections" ? <Collections onQueued={() => setView("downloads")} />
          : view === "browse" ? null
          : <Placeholder view={here.label} icon={here.icon} />}
      </main>
      {installing && <InstallDialog {...installing} premium={premium} onClose={() => setInstalling(null)} onQueued={() => setInstalling(null)} />}
      {choices && <FomodDialog need={choices} onClose={() => setChoices(null)} />}
      {existingOpen && <ExistingDialog onClose={() => setExistingOpen(false)} />}
      {onboard && <Onboarding onDone={() => { setOnboard(false); game.health().then((h) => setHealthIssues(h.problems)).catch(() => {}); }} />}
      <Toasts />
    </div>
  );
}

function Placeholder({ view, icon }: { view: string; icon: IconName }) {
  return (
    <section className="panel">
      <header className="ph"><span className="pt"><Icon name={icon} />{view}</span></header>
      <div className="pb"><Empty icon={icon} title={`${view} is not built yet`}>It arrives in a later milestone.</Empty></div>
    </section>
  );
}
