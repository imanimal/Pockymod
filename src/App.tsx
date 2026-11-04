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
import { Toasts, toast } from "./ui/toast";
import { errorText, nexus, nexusPages, onToast, queue, type QueueSnapshot } from "./lib/api";
import { applyTheme, getThemeChoice, setThemeChoice, watchSystemTheme } from "./lib/theme";
import { windowAction } from "./lib/tauri";
import { RateLcd } from "./ui/RateLcd";

export type View = "browse" | "library" | "collections" | "downloads" | "loadorder" | "health" | "settings" | "gallery";

const VIEWS: Array<{ id: View; label: string; icon: IconName }> = [
  { id: "browse", label: "Browse", icon: "browse" },
  { id: "library", label: "Library", icon: "library" },
  { id: "collections", label: "Collections", icon: "collections" },
  { id: "downloads", label: "Downloads", icon: "downloads" },
  { id: "loadorder", label: "Load order", icon: "loadorder" },
  { id: "health", label: "Health", icon: "health" },
  { id: "settings", label: "Settings", icon: "settings" },
];

export function App() {
  const [view, setView] = useState<View>("browse");
  const [scheme, setScheme] = useState<"cream" | "night">(() => applyTheme());
  const [snap, setSnap] = useState<QueueSnapshot | null>(null);
  const [premium, setPremium] = useState(false);
  const [installing, setInstalling] = useState<{ modId: number; fileId: number; name: string } | null>(null);
  const [updates] = useState(0);
  const [healthIssues] = useState(0);

  useEffect(() => watchSystemTheme(() => setScheme(applyTheme())), []);
  useEffect(() => {
    const offs: Array<() => void> = [];
    queue.get().then(setSnap).catch(() => {});
    queue.onChange(setSnap).then((u) => offs.push(u));
    onToast((m) => toast(m)).then((u) => offs.push(u));
    nexus.status().then((s) => setPremium(!!s.user?.isPremium)).catch(() => {});
    return () => offs.forEach((f) => f());
  }, []);

  const s = snap?.summary;
  const current = snap?.items.find((i) => i.state.state === "downloading");
  const progress = current && current.state.state === "downloading" && current.state.total ? current.state.received / current.state.total : undefined;

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
              count={v.id === "library" ? updates : v.id === "downloads" ? s?.left : undefined}
              countHot={v.id === "library"}
              progress={v.id === "downloads" && s?.running ? progress ?? 0 : undefined} />
          ))}
        </div>
        <div className="foot">
          <RateLcd />
          <ProgramKey icon="gallery" label="Gallery" on={view === "gallery"} onClick={() => setView("gallery")} />
        </div>
      </nav>

      <main className="view">
        {/* Browse stays mounted so its search and filters survive a trip to another view */}
        <div style={{ display: view === "browse" ? "contents" : "none" }}>
          <Browse onInstall={(d, fileId) => setInstalling({ modId: d.detail.modId, fileId, name: d.detail.name })}
            onOpenPage={(modId) => nexusPages.mod(modId).catch((e) => toast(errorText(e), true))} />
        </div>
        {view === "gallery" ? <Gallery />
          : view === "settings" ? <Settings scheme={scheme} onScheme={setScheme} />
          : view === "downloads" ? <Downloads snap={snap} />
          : view === "library" ? <Library />
          : view === "browse" ? null
          : <Placeholder view={here.label} icon={here.icon} />}
      </main>
      {installing && <InstallDialog {...installing} premium={premium} onClose={() => setInstalling(null)} onQueued={() => setInstalling(null)} />}
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
