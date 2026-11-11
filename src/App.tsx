import { useEffect, useState } from "react";
import { Icon } from "./ui/Icon";
import { Logo } from "./ui/Logo";
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
import { applyTextSize, installZoomKeys } from "./lib/zoom";
import { DropZone } from "./ui/DropZone";
import { FomodDialog } from "./screens/FomodDialog";
import { Create } from "./screens/Create";
import { Addons } from "./screens/Addons";
import { Toasts, toast } from "./ui/toast";
import { installPrefs } from "./lib/prefs";
import { background as backgroundApi, errorText, game, handler, nexus, nexusPages, onChoicesNeeded, onToast, queue, settings, type ChoicesNeeded, updates as updatesApi, type QueueSnapshot, type Update } from "./lib/api";
import { toggleScheme, useTheme, watchSystemTheme } from "./lib/theme";
import { themeBack } from "./ui/themeEscape";
import { channel, powerOff, powerOn } from "./ui/fx";
import { nope } from "./ui/motion";
import { call, held, inTauri, listen, windowAction } from "./lib/tauri";
import { RateLcd } from "./ui/RateLcd";
import { BrowserPane } from "./ui/BrowserPane";
import { useAfterPlayNudge } from "./ui/Endorse";
import { SHORTCUTS, installKeys } from "./lib/keys";
import { installGamepad } from "./lib/gamepad";
import type { Go } from "./lib/nav";
import { Dialog } from "./ui/Dialog";

export type View = "browse" | "library" | "collections" | "downloads" | "loadorder" | "health" | "settings" | "create" | "gallery" | "addons";

const VIEWS: Array<{ id: View; label: string; icon: IconName }> = [
  { id: "browse", label: "Browse", icon: "browse" },
  { id: "library", label: "Library", icon: "library" },
  { id: "collections", label: "Collections", icon: "collections" },
  { id: "downloads", label: "Downloads", icon: "downloads" },
  { id: "loadorder", label: "Load order", icon: "loadorder" },
  { id: "health", label: "Health", icon: "health" },
  { id: "create", label: "Create", icon: "share" },
  { id: "addons", label: "Add-ons", icon: "addons" },
  { id: "settings", label: "Settings", icon: "settings" },
];

export function App() {
  // the app comes back where it was (a window made again from the tray, a restart after an update)
  const [view, setView] = useState<View>(() => { try { const v = localStorage.getItem("pm.session.view"); return VIEWS.some((x) => x.id === v) ? v as View : "browse"; } catch { return "browse"; } });
  useEffect(() => { try { localStorage.setItem("pm.session.view", view); } catch { /* storage unavailable */ } }, [view]);
  const theme = useTheme();
  const [snap, setSnap] = useState<QueueSnapshot | null>(null);
  const [premium, setPremium] = useState(false);
  const [installing, setInstalling] = useState<{ modId: number; fileId: number; name: string } | null>(null);
  const [updates, setUpdates] = useState<Update[]>([]);
  const [healthIssues, setHealthIssues] = useState(0);
  const [existingOpen, setExistingOpen] = useState(false);
  const [onboard, setOnboard] = useState(false);
  const [choices, setChoices] = useState<ChoicesNeeded | null>(null);
  useEffect(() => held(onChoicesNeeded(setChoices)), []);
  useEffect(() => { settings.get("onboarded").then((v) => setOnboard(v !== "1")).catch(() => {}); }, []);

  useEffect(() => watchSystemTheme(() => {}), []);
  useEffect(() => {
    channel();
    // while the new view settles in, its scrollbars stay out of sight (fx.css), so none flashes for the slide
    const v = document.querySelector(".shell > .view");
    v?.classList.add("entering");
    const t = setTimeout(() => v?.classList.remove("entering"), 900);
    return () => { clearTimeout(t); v?.classList.remove("entering"); };
  }, [view]);
  // closing the window: the picture turns off like a tube, then the app quits (src-tauri/src/power.rs)
  useEffect(() => {
    return held(listen<{ seq: number }>("power-off", (p) => powerOff(() => { void call("power_done", { seq: p.seq }).catch(() => {}); })));
  }, []);
  useEffect(() => installUndoKeys(), []);
  useAfterPlayNudge();
  const [help, setHelp] = useState(false);
  // shortcuts and the controller (lib/keys.ts, lib/gamepad.ts)
  useEffect(() => {
    const ids = VIEWS.map((v) => v.id);
    const go = (v: string) => setView(v as View);
    const back = () => window.dispatchEvent(new Event("pm-back"));
    const offKeys = installKeys({ views: ids, go, help: () => setHelp((h) => !h), back, pane: () => window.dispatchEvent(new Event("pm-pane-toggle")) });
    const offPad = installGamepad({
      nextView: (d) => setView((cur) => ids[(ids.indexOf(cur) + d + ids.length) % ids.length] as View),
      help: () => setHelp((h) => !h), back, themeBack,
      play: () => (document.querySelector(".side .foot .btn") as HTMLElement | null)?.focus(),
    });
    return () => { offKeys(); offPad(); };
  }, []);
  // a link in a toast or a note goes to a view, and to a section of it (lib/nav.ts)
  useEffect(() => {
    let t: number | undefined;
    const h = (e: Event) => {
      const { view: to, section } = (e as CustomEvent<Go>).detail;
      if (!VIEWS.some((x) => x.id === to)) return;
      setView(to as View);
      if (!section) return;
      window.clearTimeout(t);
      t = window.setTimeout(() => {
        const title = [...document.querySelectorAll<HTMLElement>(".panel > .ph .pt")].find((x) => x.textContent?.trim() === section);
        const panel = title?.closest<HTMLElement>(".panel");
        panel?.scrollIntoView({ block: "start", behavior: "smooth" });
        panel?.classList.remove("go-flash"); void panel?.offsetWidth; panel?.classList.add("go-flash");
      }, 500);
    };
    window.addEventListener("pm-go", h);
    return () => { window.removeEventListener("pm-go", h); window.clearTimeout(t); };
  }, []);
  // the way out of a theme that breaks the app: Ctrl+Alt+Shift+T goes back a theme, wherever the focus is
  useEffect(() => {
    const h = (e: KeyboardEvent) => { if (e.ctrlKey && e.altKey && e.shiftKey && e.key.toLowerCase() === "t") { e.preventDefault(); themeBack(); } };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, []);
  useEffect(() => { applyTextSize(); return installZoomKeys((px) => toast(`Text size ${px}px (Ctrl+0 resets)`)); }, []);
  useEffect(() => {
    const offs: Array<() => void> = [];
    queue.get().then(setSnap).catch(() => {});
    offs.push(held(queue.onChange(setSnap)), held(onToast((m) => toast(m))));
    // a new version is downloaded: one quiet line, never a dialog
    offs.push(held(backgroundApi.onUpdate((u) => { if (u.ready) toast(`Pockymod ${u.version} installs when you close the window`, false, { label: "Restart now", run: () => { void backgroundApi.installUpdate().catch((e) => toast(errorText(e), true)); } }); })));
    nexus.status().then((s) => setPremium(!!s.user?.isPremium)).catch(() => {});
    // the account went Premium (ui/Premium.tsx): installs stop counting clicks
    const upgraded = () => setPremium(true);
    window.addEventListener("pm-premium", upgraded);
    offs.push(() => window.removeEventListener("pm-premium", upgraded));
    // update badges: the last result now and a fresh check shortly after the window opens; every six hours only
    // when the user turned that on (Settings, Background), since Nexus has a key used only for what the user starts
    updatesApi.last().then(setUpdates).catch(() => {});
    offs.push(held(updatesApi.onChange(setUpdates)));
    const check = () => updatesApi.check().catch(() => {});
    const t1 = setTimeout(check, 4000);
    const t2 = setInterval(() => { settings.get("mods.background").then((v) => { if (v === "1") void check(); }).catch(() => {}); }, 6 * 3600 * 1000);
    game.health().then((h) => setHealthIssues(h.problems)).catch(() => {});
    // another manager (often Vortex on its own start) took the browser's download links: offer them back once
    const t3 = setTimeout(() => {
      Promise.all([handler.status(), settings.get("onboarded"), settings.get("links.other")]).then(([h, ob, other]) => {
        if (ob !== "1" || other === "1" || !h.supported || h.ours || !h.owner) return;
        toast(`${h.owner} took over Mod Manager Download links`, false, { label: "Take back", run: () => { void handler.register().then(() => toast("Links open in Pockymod")).catch((e) => toast(errorText(e), true)); } });
      }).catch(() => {});
    }, 2500);
    return () => { offs.forEach((f) => f()); clearTimeout(t1); clearInterval(t2); clearTimeout(t3); };
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
      const extra = snap.items.filter((i) => i.requiredBy === name && ["waiting", "needsClick", "downloading"].includes(i.state.state)).length;
      toast(`Queued ${name}${extra ? ` and ${extra} required` : ""}`);
    } catch (e) { toast(errorText(e), true); }
  };


  const led = (id: View): Led | undefined =>
    id === "downloads" ? (s?.running ? "run" : s?.waitingOnUser ? "wait" : "off") : id === "health" && healthIssues > 0 ? "bad" : undefined;

  const here = [...VIEWS, { id: "gallery" as View, label: "Gallery", icon: "gallery" as IconName }].find((v) => v.id === view)!;

  return (
    <div className="screen shell">
      <header className="mtop" data-tauri-drag-region onDoubleClick={(e) => { if (e.target === e.currentTarget) void windowAction("toggleMaximize"); }}>
        <span className="brandmark" data-tauri-drag-region><Logo /><span>Pockymod</span></span>
        <span className="crumb" data-tauri-drag-region><span className="v">{here.label}</span><span className="ctx">Cyberpunk 2077</span></span>
        <span className="sp" data-tauri-drag-region />
        <HistoryMenu onRemoved={() => { setView("library"); setTimeout(() => document.getElementById("recently-removed")?.scrollIntoView({ behavior: "smooth" }), 120); }} />
        <ProfileSwitch onManage={() => setView("loadorder")} />
        <Key variant="ghost" size="sm" icon={theme.scheme === "dark" ? "sun" : "moon"}
          onClick={(e) => { if (!toggleScheme()) { nope(e.currentTarget); toast(`${theme.name} is dark only`); } }}
          aria-label={theme.scheme === "dark" ? "Switch to light" : "Switch to dark"} title={`${theme.name}, ${theme.scheme}`} />
        <span className="winkeys">
          <button type="button" className="wk" aria-label="Minimise" title="Minimise" onClick={() => void windowAction("minimize")}><Icon name="minus" size={9} /></button>
          <button type="button" className="wk" aria-label="Maximise" title="Maximise" onClick={() => void windowAction("toggleMaximize")}><Icon name="max" size={9} /></button>
          <button type="button" className="wk wk-x" aria-label="Close" title="Close" onClick={() => { if (inTauri()) void windowAction("close"); else powerOff(() => setTimeout(() => powerOn(), 700)); }}><Icon name="x" size={9} /></button>
        </span>
      </header>

      <nav className="side" aria-label="Views">
        <div className="nkeys rail">
          {VIEWS.map((v) => (
            <ProgramKey key={v.id} icon={v.icon} label={v.label} on={view === v.id} onClick={() => setView(v.id)} title={`${v.label} (Ctrl+${VIEWS.indexOf(v) + 1})`}
              led={led(v.id)}
              count={v.id === "library" ? updates.length : v.id === "downloads" ? s?.left : undefined}
              countHot={v.id === "library"}
              progress={v.id === "downloads" && s?.running ? progress ?? 0 : undefined} />
          ))}
        </div>
        <div className="foot">
          <LaunchDeck onHealth={() => setView("health")} />
          <RateLcd />
        </div>
      </nav>

      <main className="view">
        {/* Browse stays mounted so its search and filters survive a trip to another view */}
        <div className="keep" style={{ display: view === "browse" ? "contents" : "none" }}>
          <Browse onInstall={(d, fileId) => void startInstall(d.detail.modId, fileId, d.detail.name)} updates={updates} onLibrary={() => setView("library")}
            onOpenPage={(modId) => nexusPages.mod(modId).catch((e) => toast(errorText(e), true))} />
        </div>
        {view === "gallery" ? <Gallery />
          : view === "settings" ? <Settings onGallery={() => setView("gallery")} />
          : view === "downloads" ? <Downloads snap={snap} />
          : view === "library" ? <Library updates={updates} onExisting={() => setExistingOpen(true)} />
          : view === "health" ? <Health onNavigate={setView} onIssues={setHealthIssues} />
          : view === "loadorder" ? <LoadOrder />
          : view === "create" ? <Create />
          : view === "addons" ? <Addons onNavigate={setView} />
          : view === "collections" ? <Collections onQueued={() => setView("downloads")} />
          : view === "browse" ? null
          : <Placeholder view={here.label} icon={here.icon} />}
      </main>
      {installing && <InstallDialog {...installing} premium={premium} onClose={() => setInstalling(null)} onQueued={() => setInstalling(null)} />}
      {choices && <FomodDialog need={choices} onClose={() => setChoices(null)} />}
      {existingOpen && <ExistingDialog onClose={() => setExistingOpen(false)} />}
      {onboard && <Onboarding onDone={() => { setOnboard(false); game.health().then((h) => setHealthIssues(h.problems)).catch(() => {}); }} />}
      <BrowserPane waiting={!!s?.waitingOnUser} />
      {help && (
        <Dialog icon="key" title="Shortcuts" onClose={() => setHelp(false)}>
          <div className="keys-list">{SHORTCUTS.map(([k, what]) => <div key={k}><kbd>{k}</kbd><span>{what}</span></div>)}</div>
          <div className="sub-h">Controller</div>
          <div className="keys-list">
            {[["D-pad, left stick", "Move between keys"], ["A", "Press"], ["B", "Back, close"], ["X", "Install, or the mod's menu"], ["Y", "Find"],
              ["LB, RB", "Previous, next view"], ["LT, RT, right stick", "Scroll"], ["View", "This list"], ["Menu", "Go to Play"], ["L3+R3", "Go back to the last theme"]].map(([k, what]) => <div key={k}><kbd>{k}</kbd><span>{what}</span></div>)}
          </div>
        </Dialog>
      )}
      <div className="padbar" aria-hidden="true"><span><kbd>A</kbd> press</span><span><kbd>B</kbd> back</span><span><kbd>X</kbd> install or menu</span><span><kbd>Y</kbd> find</span><span><kbd>LB</kbd><kbd>RB</kbd> views</span><span><kbd>View</kbd> all buttons</span></div>
      <DropZone onChoices={setChoices} />
      <Toasts />
    </div>
  );
}

function Placeholder({ view, icon }: { view: string; icon: IconName }) {
  return (
    <section className="panel">
      <header className="ph"><span className="pt"><Icon name={icon} />{view}</span></header>
      <div className="pb"><Empty icon={icon} title={`${view} isn't built yet`} /></div>
    </section>
  );
}
