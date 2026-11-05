// The shell: the window frame (title bar with the rate-limit LCD and window keys), the program-key rail,
// the current screen, and the key deck with the screen's own keys.

import { useEffect, useState } from "react";
import { Icon } from "./components/Icon";
import { Key, Lcd, ProgramKey } from "./components/pocket";
import { api, inTauri, windowAction } from "./lib/api";
import { count } from "./lib/format";
import { applyTheme, useApp, type View } from "./lib/store";
import { Browse } from "./screens/Browse";
import { Collections } from "./screens/Collections";
import { Downloads } from "./screens/Downloads";
import { Gallery } from "./screens/Gallery";
import { Health } from "./screens/Health";
import { Library } from "./screens/Library";
import { LoadOrder } from "./screens/LoadOrder";
import { Settings } from "./screens/Settings";

const TITLES: Record<View, [string, string]> = {
  browse: ["Browse", "Find mods on Nexus without leaving the app"],
  library: ["Library", "What's installed, per profile"],
  collections: ["Collections", "Install a collection, or share your own"],
  downloads: ["Downloads", "The download queue"],
  loadorder: ["Load order", "REDmods, first loads first"],
  health: ["Health", "What needs fixing before you play"],
  settings: ["Settings", "Account, game folder, and how the app behaves"],
  gallery: ["Component gallery", "Every Pocket part this app uses"],
};

/** The rate-limit LCD in the title bar: green normal, amber low, red throttled. */
function RateLcd() {
  const { status } = useApp();
  const r = status?.rate;
  const tone = r?.level === "throttled" ? "bad" : r?.level === "low" ? "hot" : null;
  const daily = (r?.dailyRemaining ?? 1) > 0;
  return (
    <div className="ratelcd" title={r ? `Nexus requests left: ${r.dailyRemaining ?? "?"} today, ${r.hourlyRemaining ?? "?"} this hour` : "Nexus requests left"}>
      <Lcd label={daily ? "Nexus today" : "Nexus /hour"} value={status?.hasKey && r?.remaining != null ? count(r.remaining) : "--"} tone={tone} className="clock" />
    </div>
  );
}

export function Shell() {
  const { view, go, status, toasts, dismiss, healthProblems } = useApp();
  const [settingsTheme, setTheme] = useState<"system" | "cream" | "night">("system");

  useEffect(() => {
    api.settings().then((s) => {
      setTheme(s.theme);
      applyTheme(s.theme);
      document.documentElement.dataset.motion = s.motion ? "1" : "0";
    });
    const mq = window.matchMedia?.("(prefers-color-scheme: dark)");
    const onChange = () => applyTheme(settingsTheme);
    mq?.addEventListener?.("change", onChange);
    return () => mq?.removeEventListener?.("change", onChange);
  }, [settingsTheme]);

  // once per launch: a newer Pocket Mods release, and updates for installed mods
  const { toast, go: goTo } = useApp();
  const [checked, setChecked] = useState(false);
  useEffect(() => {
    if (checked || !status) return;
    setChecked(true);
    api.settings().then((s) => {
      if (s.checkAppUpdates) api.appUpdateCheck().then((r) => r?.newer && toast(`Pocket Mods ${r.version} is out. See Settings, Updates.`)).catch(() => {});
    });
    if (status.hasKey) api.checkUpdates().then((n) => n > 0 && toast(`${n} mod update(s) available.`, { action: { label: "Library", run: () => goTo("library") } })).catch(() => {});
  }, [status, checked, toast, goTo]);

  const flipTheme = async () => {
    const dark = document.documentElement.dataset.scheme === "dark";
    const next = dark ? "cream" : "night";
    applyTheme(next);
    setTheme(next);
    const s = await api.settings();
    await api.saveSettings({ ...s, theme: next });
  };

  const q = status?.queue;
  const [title, ctx] = TITLES[view];
  const keys: { v: View; icon: Parameters<typeof ProgramKey>[0]["icon"]; label: string; extra?: Partial<Parameters<typeof ProgramKey>[0]> }[] = [
    { v: "browse", icon: "browse", label: "Browse" },
    { v: "library", icon: "library", label: "Library", extra: { count: status?.updates, countHot: true } },
    { v: "collections", icon: "collections", label: "Collections" },
    {
      v: "downloads",
      icon: "download",
      label: "Downloads",
      extra: {
        count: q?.left,
        led: q?.running ? "on" : q?.waitingOnUser ? "wait" : "off",
        progress: q && q.left > 0 ? q.percent : null,
      },
    },
    { v: "loadorder", icon: "order", label: "Load order" },
    { v: "health", icon: "health", label: "Health", extra: { count: healthProblems } },
    { v: "settings", icon: "settings", label: "Settings" },
  ];

  return (
    <div className="screen bset-b8 fset-f3 content">
      <header className="mtop" data-tauri-drag-region>
        <div className="brandmark" data-tauri-drag-region>
          <span className="logo" aria-hidden>
            P
          </span>
          <span>Pocket Mods</span>
        </div>
        <div className="crumb" data-tauri-drag-region>
          <span className="v">{title}</span>
          <span className="ctx">{ctx}</span>
        </div>
        <span className="sp" data-tauri-drag-region />
        <RateLcd />
        <Key variant="ghost" icon={document.documentElement.dataset.scheme === "dark" ? "sun" : "moon"} onClick={flipTheme} title="Cream or Night" aria-label="Switch theme" />
        {inTauri ? (
          <div className="winkeys">
            <button className="wk" onClick={() => windowAction("minimize")} aria-label="Minimize">
              <Icon name="min" />
            </button>
            <button className="wk" onClick={() => windowAction("maximize")} aria-label="Maximize">
              <Icon name="max" />
            </button>
            <button className="wk wk-x" onClick={() => windowAction("close")} aria-label="Close">
              <Icon name="close" />
            </button>
          </div>
        ) : null}
      </header>
      <nav className="mnav" aria-label="Views">
        <div className="nkeys">
          {keys.map((k) => (
            <ProgramKey key={k.v} icon={k.icon} label={k.label} on={view === k.v} onClick={() => go(k.v)} {...k.extra} />
          ))}
        </div>
      </nav>
      <main className="page" id="page">
        <ViewSwitch />
      </main>
      <footer className="deck" id="deck">
        <div className="deck-keys" id="deck-keys" />
        <span className="deck-sp" />
        <span className="speaker" aria-hidden />
      </footer>
      <div className="toasts" role="status" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast ${t.tone ?? ""}`}>
            <span className="msg">
              <Icon name={t.tone === "bad" ? "warn" : "info"} />
              <span>{t.message}</span>
            </span>
            {t.action ? (
              <Key size="sm" onClick={t.action.run}>
                {t.action.label}
              </Key>
            ) : null}
            <Key variant="ghost" size="sm" icon="close" aria-label="Dismiss" onClick={() => dismiss(t.id)} />
          </div>
        ))}
      </div>
    </div>
  );
}

function ViewSwitch() {
  const { view } = useApp();
  switch (view) {
    case "browse":
      return <Browse />;
    case "library":
      return <Library />;
    case "collections":
      return <Collections />;
    case "downloads":
      return <Downloads />;
    case "loadorder":
      return <LoadOrder />;
    case "health":
      return <Health />;
    case "settings":
      return <Settings />;
    case "gallery":
      return <Gallery />;
  }
}
