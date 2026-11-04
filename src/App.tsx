import { useEffect, useState } from "react";
import { Icon } from "./ui/Icon";
import type { IconName } from "./ui/icons";
import { Empty, Key, ProgramKey, type Led } from "./ui/pocket";
import { Gallery } from "./screens/Gallery";
import { Settings } from "./screens/Settings";
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

export interface ShellStatus {
  queueRunning: boolean;
  queueProgress?: number;
  queueCount: number;
  updates: number;
  healthIssues: number;
}

export function App() {
  const [view, setView] = useState<View>("browse");
  const [scheme, setScheme] = useState<"cream" | "night">(() => applyTheme());
  const [status] = useState<ShellStatus>({ queueRunning: false, queueCount: 0, updates: 0, healthIssues: 0 });

  useEffect(() => watchSystemTheme(() => setScheme(applyTheme())), []);

  const toggleScheme = () => {
    const next = scheme === "night" ? "cream" : "night";
    setThemeChoice(next);
    setScheme(next);
  };

  const led = (id: View): Led | undefined =>
    id === "downloads" ? (status.queueRunning ? "run" : "off") : id === "health" && status.healthIssues > 0 ? "bad" : undefined;

  const current = [...VIEWS, { id: "gallery" as View, label: "Gallery", icon: "gallery" as IconName }].find((v) => v.id === view)!;

  return (
    <div className="screen shell">
      <header className="mtop" data-tauri-drag-region onDoubleClick={(e) => { if (e.target === e.currentTarget) void windowAction("toggleMaximize"); }}>
        <span className="brandmark" data-tauri-drag-region><span className="logo">P</span><span>Pocket Mods</span></span>
        <span className="crumb" data-tauri-drag-region><span className="v">{current.label}</span><span className="ctx">Cyberpunk 2077</span></span>
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
              count={v.id === "library" ? status.updates : v.id === "downloads" ? status.queueCount : undefined}
              countHot={v.id === "library"}
              progress={v.id === "downloads" && status.queueRunning ? status.queueProgress : undefined} />
          ))}
        </div>
        <div className="foot">
          <RateLcd />
          <ProgramKey icon="gallery" label="Gallery" on={view === "gallery"} onClick={() => setView("gallery")} />
        </div>
      </nav>

      <main className="view">
        {view === "gallery" ? <Gallery />
          : view === "settings" ? <Settings scheme={scheme} onScheme={setScheme} />
          : <Placeholder view={current.label} icon={current.icon} />}
      </main>
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
