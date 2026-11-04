// The browser pane: Nexus pages inside the main window, beside the app, when Settings says pages open in the app.
// The page itself is a native webview (src-tauri/src/browser.rs) laid over .bpane-view; this part draws the pane's
// frame and keys and tells Rust where that box is. It sits beside the app (split, with a handle to resize), over the
// whole view (full), behind a tab at the window's edge (hidden: the page keeps its place), or in a window of its
// own (popped out). A native webview draws above the app, so it steps aside while one of the app's own windows,
// menus or drawers would be under it, and while the window turns off.
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { Icon } from "./Icon";
import { Key } from "./pocket";
import { toast } from "./toast";
import { browserPane, errorText, type BrowserState } from "../lib/api";
import { inTauri } from "../lib/tauri";

type Mode = "split" | "full" | "hidden";
const MODE = "pm.pane.mode", WIDTH = "pm.pane.width";
const read = (k: string) => { try { return localStorage.getItem(k); } catch { return null; } };
const write = (k: string, v: string) => { try { localStorage.setItem(k, v); } catch { /* storage unavailable */ } };
/** The app's own layers that a native webview would hide. */
const OVERLAYS = ".ov, .drawer-ov, .lightbox, .pmenu, .hmenu";

export function BrowserPane({ waiting }: { waiting?: boolean }) {
  const [st, setSt] = useState<BrowserState>({ open: false, popped: false, url: null, title: null, loading: false, reveal: false });
  const [title, setTitle] = useState<string | null>(null);
  const [mode, setModeState] = useState<Mode>(() => (read(MODE) === "full" ? "full" : "split"));
  const [width, setWidthState] = useState(() => Math.max(360, Number(read(WIDTH)) || 620));
  const [covered, setCovered] = useState(false);
  const [off, setOff] = useState(false);
  const view = useRef<HTMLDivElement>(null);
  const sent = useRef("");
  const setMode = (m: Mode) => { setModeState(m); if (m !== "hidden") write(MODE, m); };

  useEffect(() => {
    const offs: Array<() => void> = [];
    browserPane.state().then(setSt).catch(() => {});
    browserPane.onChange((s) => {
      setSt(s);
      if (!s.open) setTitle(null);
      // a page the app opened (one waiting for your click) brings a hidden pane back
      if (s.reveal && !s.popped) setModeState((m) => (m === "hidden" ? ((read(MODE) as Mode | null) === "full" ? "full" : "split") : m));
    }).then((u) => offs.push(u));
    browserPane.onTitle((t) => setTitle(t.title)).then((u) => offs.push(u));
    const pw = () => setOff(true);
    window.addEventListener("pm-power-off", pw);
    return () => { offs.forEach((f) => f()); window.removeEventListener("pm-power-off", pw); };
  }, []);

  const docked = st.open && !st.popped;
  const showing = docked && mode !== "hidden";
  // the shell makes room for the pane (fx.css has nothing to do with it; app.css .with-pane / .pane-full)
  useLayoutEffect(() => {
    const sh = document.querySelector<HTMLElement>(".shell");
    if (!sh) return;
    sh.classList.toggle("with-pane", showing && mode === "split");
    sh.classList.toggle("pane-full", showing && mode === "full");
    sh.style.setProperty("--pane-w", `${width}px`);
    return () => { sh.classList.remove("with-pane", "pane-full"); };
  }, [showing, mode, width]);

  // where the page goes, whenever the box moves or the app's own layers come and go over it
  // only a change is sent: the page changes often (downloads tick, toasts come and go), the pane's box rarely
  const send = (x: number, y: number, w: number, h: number, v: boolean) => {
    const k = `${Math.round(x)},${Math.round(y)},${Math.round(w)},${Math.round(h)},${v}`;
    if (k === sent.current) return;
    sent.current = k;
    void browserPane.bounds(x, y, w, h, v).catch(() => { sent.current = ""; });
  };
  const place = useCallback(() => {
    const el = view.current;
    if (!inTauri()) return;
    if (!el || !showing) { send(0, 0, 0, 0, false); return; }
    const r = el.getBoundingClientRect();
    const over = [...document.querySelectorAll(OVERLAYS)].some((o) => {
      const b = o.getBoundingClientRect();
      return b.width > 0 && b.height > 0 && b.left < r.right && b.right > r.left && b.top < r.bottom && b.bottom > r.top;
    });
    setCovered(over);
    send(r.left, r.top, r.width, r.height, !over && !off);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showing, off]);
  // over the whole view, a key in the rail means "show me that": the pane goes back beside it
  useEffect(() => {
    if (!showing || mode !== "full") return;
    const h = (e: MouseEvent) => { if ((e.target as Element | null)?.closest?.(".side .nv")) setMode("split"); };
    document.addEventListener("click", h, true);
    return () => document.removeEventListener("click", h, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showing, mode]);
  useLayoutEffect(() => {
    place();
    const el = view.current;
    const ro = new ResizeObserver(place);
    if (el) ro.observe(el);
    const sh = document.querySelector(".shell");
    if (sh) ro.observe(sh);
    const mo = new MutationObserver(() => requestAnimationFrame(place));
    mo.observe(document.body, { childList: true, subtree: true });
    window.addEventListener("resize", place);
    return () => { ro.disconnect(); mo.disconnect(); window.removeEventListener("resize", place); };
  }, [place]);

  const act = (p: Promise<unknown>) => p.catch((e) => toast(errorText(e), true));
  // the handle between the app and the pane: drag to resize, double-click for half the window
  const drag = (e: React.PointerEvent) => {
    e.preventDefault();
    const x0 = e.clientX, w0 = width;
    const move = (ev: PointerEvent) => setWidthState(Math.max(360, Math.min(window.innerWidth - 420, w0 + (x0 - ev.clientX))));
    const up = () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); setWidthState((w) => { write(WIDTH, String(w)); return w; }); };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  if (!st.open) return null;
  if (st.popped || mode === "hidden") return (
    <button type="button" className="bpane-tab" onClick={() => (st.popped ? act(browserPane.pop(false)) : setMode((read(MODE) as Mode | null) === "full" ? "full" : "split"))}
      title={st.popped ? "The Nexus window is open on its own. Bring it back into this window." : "Show the Nexus pane again"}>
      <Icon name="web" size={9} /><span>{st.popped ? "Nexus window" : "Nexus"}</span>{waiting && <span className="led wait" aria-label="Waiting for your click" />}
    </button>
  );
  let host = "";
  try { host = st.url ? new URL(st.url).host.replace(/^www\./, "") : ""; } catch { /* not a full address */ }
  return (
    <aside className={`bpane${mode === "full" ? " full" : ""}`} aria-label="Nexus pages">
      {mode === "split" && <div className="bpane-split" role="separator" aria-orientation="vertical" aria-label="Resize the Nexus pane" onPointerDown={drag}
        onDoubleClick={() => { const w = Math.round(window.innerWidth / 2); setWidthState(w); write(WIDTH, String(w)); }} />}
      <div className="bpane-bar">
        <Key size="sm" variant="ghost" icon="chevron" className="flip" onClick={() => act(browserPane.nav("back"))} aria-label="Back" title="Back" />
        <Key size="sm" variant="ghost" icon="chevron" onClick={() => act(browserPane.nav("forward"))} aria-label="Forward" title="Forward" />
        <Key size="sm" variant="ghost" icon="refresh" onClick={() => act(browserPane.nav(st.loading ? "stop" : "reload"))} aria-label={st.loading ? "Stop" : "Reload"} title={st.loading ? "Stop" : "Reload"} />
        <button type="button" className="bpane-url" title={st.url ? `${st.url} (click to copy)` : undefined}
          onClick={() => st.url && navigator.clipboard?.writeText(st.url).then(() => toast("Address copied.")).catch(() => {})}>
          {st.loading && <span className="led run" aria-label="Loading" />}
          <b>{title || host || "Nexus Mods"}</b>{host && <span className="dim">{host}</span>}
        </button>
        <Key size="sm" variant="ghost" icon="link" onClick={() => act(browserPane.external())} aria-label="Open in your browser"
          title="This page in your own browser: you are signed in there, with your saved passwords and extensions" />
        <Key size="sm" variant="ghost" icon="max" latched={mode === "full"} onClick={() => setMode(mode === "full" ? "split" : "full")}
          aria-label={mode === "full" ? "Beside the app" : "Over the whole view"} title={mode === "full" ? "Beside the app" : "Over the whole view"} />
        <Key size="sm" variant="ghost" icon="share" onClick={() => act(browserPane.pop(true))} aria-label="Pop out" title="Pop out into a window of its own (still signed in)" />
        <Key size="sm" variant="ghost" icon="minus" onClick={() => setMode("hidden")} aria-label="Hide" title="Hide behind a tab; the page keeps its place" />
        <Key size="sm" variant="ghost" icon="x" onClick={() => act(browserPane.close())} aria-label="Close the Nexus pane" title="Close" />
      </div>
      <div className="bpane-view" ref={view}>
        {!inTauri() && <div className="bpane-stub"><Icon name="web" size={27} /><b>{st.url}</b><span className="dim">The Nexus page shows here in the desktop app.</span></div>}
        {inTauri() && covered && <div className="bpane-stub"><Icon name="web" size={27} /><span className="dim">The page steps aside while this window is open.</span></div>}
      </div>
    </aside>
  );
}
