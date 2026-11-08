// The way back from a theme that breaks the app. When an installed theme comes on (and at the first few launches in
// it), a notice in the toasts' corner says which key goes back to the theme before, and has a key that does it. It is
// drawn like a toast, but a theme's css reaches every element of the page, so the notice is built where it can't:
//   a shadow root whose :host rule resets everything with !important (inside a shadow root, that beats the page's own
//   !important rules), shown in the top layer, above anything the page stacks whatever its z-index, transform or
//   opacity. <html> is held displayed while it shows and a zoom on <html> is undone. The colours are the theme's only
//   when they read (4.5:1 for text), else Redline's, and the shape is the look's own, never the theme's.
import { BUILTIN_THEMES, goBackTheme, previousTheme, themeState, type Look } from "../lib/theme";
import { errorText } from "../lib/api";
import { iconRects } from "./icons";
import { LINE_ICONS } from "./icons-line";
import { toast } from "./toast";

export const ESCAPE_KEY = "Ctrl+Alt+Shift+T";
const SHOW_MS = 12_000;
/** At launch in an installed theme, the notice shows for the first few starts after it was picked. */
const LAUNCHES = 3;
const SEEN = "pm.theme.escape";

/** Back to the theme before this one: the escape key, both sticks on a controller, and the notice's own key. */
export function themeBack() {
  hide();
  goBackTheme().then((to) => toast(to ? `Back to ${to.name}` : "No theme to go back to")).catch((e) => toast(errorText(e), true));
}

const ownTheme = () => !BUILTIN_THEMES.some((t) => t.id === themeState().id);

type Seen = { id: string; n: number };
function seen(): Seen | null {
  try { const s = JSON.parse(localStorage.getItem(SEEN) ?? "null") as Seen | null; return s && typeof s.id === "string" && typeof s.n === "number" ? s : null; } catch { return null; }
}
function setSeen(s: Seen) { try { localStorage.setItem(SEEN, JSON.stringify(s)); } catch { /* storage unavailable */ } }

/** Show the notice whenever an installed theme comes on, and at launch in one. Call once. */
export function installThemeEscape() {
  window.addEventListener("pm-theme", (e) => {
    if ((e as CustomEvent).detail !== "theme") return;
    if (!ownTheme()) { hide(true); return; }
    setSeen({ id: themeState().id, n: 0 });
    show();
  });
  window.setTimeout(() => {
    if (!ownTheme()) return;
    const id = themeState().id, s = seen();
    const n = s?.id === id ? s.n : 0;
    if (n >= LAUNCHES) return;
    setSeen({ id, n: n + 1 });
    show();
  }, 1500);
}

// ── colours: the theme's when they read, else Redline's ──

type Rgba = [number, number, number, number];
let probe: CanvasRenderingContext2D | null = null;

/** Any CSS colour as sRGB bytes and alpha, through a 1×1 canvas. Null when it isn't a colour. */
function rgba(value: string): Rgba | null {
  if (!value) return null;
  probe ??= Object.assign(document.createElement("canvas"), { width: 1, height: 1 }).getContext("2d", { willReadFrequently: true });
  if (!probe) return null;
  probe.fillStyle = "#000"; probe.fillStyle = value; const a = probe.fillStyle;
  probe.fillStyle = "#fff"; probe.fillStyle = value;
  if (probe.fillStyle !== a) return null; // neither took it
  probe.clearRect(0, 0, 1, 1); probe.fillRect(0, 0, 1, 1);
  const d = probe.getImageData(0, 0, 1, 1).data;
  return [d[0], d[1], d[2], d[3] / 255];
}
/** A colour laid over another, so it is opaque. */
function over(top: Rgba | null, under: Rgba | null): Rgba | null {
  if (!top || !under) return null;
  const a = top[3];
  return [0, 1, 2].map((i) => Math.round(top[i] * a + under[i] * (1 - a))).concat(1) as Rgba;
}
function luminance(c: Rgba): number {
  const f = (v: number) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
  return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2]);
}
function contrast(a: Rgba, b: Rgba): number {
  const x = luminance(a), y = luminance(b);
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
const css = (c: Rgba) => `rgb(${c[0]} ${c[1]} ${c[2]})`;

interface Palette { look: Look; bg: Rgba; fg: Rgba; quiet: Rgba; line: Rgba; mark: Rgba; icon: Rgba; actBg: Rgba; actFg: Rgba }
const hex = (h: string): Rgba => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)).concat(1) as Rgba;
const REDLINE: Palette = {
  look: "redline", bg: hex("#160e14"), fg: hex("#f1d3ce"), quiet: hex("#bb7f78"), line: hex("#6a2a29"), mark: hex("#5ef6ff"),
  icon: hex("#5ef6ff"), actBg: hex("#170b0e"), actFg: hex("#ff8a7f"),
};

/** The tokens a toast uses in this look, read off <html>; Redline's whole palette when the text or the key wouldn't read. */
function palette(): Palette {
  const look = themeState().look;
  const cs = getComputedStyle(document.documentElement);
  const tok = (n: string) => rgba(cs.getPropertyValue(n).trim());
  const win = over(tok("--bg"), [0, 0, 0, 1]);
  const surface = tok(look === "pocket" ? "--surface" : "--surface-2");
  // Redline's toasts are glass: the surface at 74% over the window
  const bg = over(look === "redline" && surface ? [surface[0], surface[1], surface[2], surface[3] * 0.74] : surface, win);
  if (!bg) return REDLINE;
  const fg = over(tok(look === "redline" ? "--text-2" : "--text"), bg);
  const actBg = over(tok("--chrome-bg"), bg);
  const actFg = over(tok("--chrome-fg"), actBg);
  if (!fg || !actBg || !actFg || contrast(fg, bg) < 4.5 || contrast(actFg, actBg) < 4.5) return REDLINE;
  const or = (c: Rgba | null, min: number) => (c && contrast(c, bg) >= min ? c : fg);
  return {
    look, bg, fg, actBg, actFg,
    quiet: or(over(tok("--text-3"), bg), 3),
    line: over(tok("--line"), bg) ?? fg,
    mark: or(over(tok(look === "redline" ? "--acc2" : "--accent"), bg), 1.6),
    icon: or(over(tok(look === "redline" ? "--acc2" : look === "standard" ? "--ok" : "--accent"), bg), 3),
  };
}

/** The app's text size (lib/zoom.ts), kept to the sizes it offers. */
function textSize(): number {
  const px = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--fs"));
  return Number.isFinite(px) ? Math.min(20, Math.max(12, px)) : 14;
}

// ── the notice ──

const FONT = `"Space Grotesk", "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif`;
const MONO = `"Space Mono", "Cascadia Mono", Consolas, ui-monospace, monospace`;

function sheet(p: Palette, fs: number): string {
  const red = p.look === "redline";
  const dark = luminance(p.bg) < 0.2;
  const ring = `0 0 0 1px ${css(p.line)}`;
  const shape = red
    ? `border-radius: 0; clip-path: polygon(0 0, 100% 0, 100% calc(100% - 9px), calc(100% - 9px) 100%, 0 100%); box-shadow: inset ${ring};
       background: linear-gradient(to bottom right, transparent calc(50% - 1px), ${css(p.line)} calc(50% - 1px) calc(50% + 1px), transparent calc(50% + 1px)) 100% 100% / 9px 9px no-repeat, ${css(p.bg)};`
    : `border-radius: ${p.look === "pocket" ? 10 : 8}px; background: ${css(p.bg)};
       box-shadow: 0 ${p.look === "pocket" ? "24px 60px" : "18px 48px"} ${dark ? "rgba(0, 0, 0, .55)" : "rgba(16, 24, 40, .22)"}, ${ring};`;
  return `
:host { all: initial !important; display: block !important; position: fixed !important; inset: 0 auto auto 0 !important;
  width: 0 !important; height: 0 !important; overflow: visible !important; visibility: visible !important; opacity: 1 !important;
  content-visibility: visible !important; contain: none !important; zoom: 1 !important; direction: ltr !important; pointer-events: none !important; }
.n { position: fixed; inset: auto; margin: 0; border: 0; box-sizing: border-box; width: max-content; height: auto;
  max-width: min(52em, calc(100vw - 2.4em)); overflow: hidden; display: flex; align-items: center; gap: .7em;
  padding: .55em .6em .55em .8em; color: ${css(p.fg)}; font: 400 ${(fs * 0.92).toFixed(2)}px/1.45 ${FONT}; text-align: left;
  -webkit-font-smoothing: antialiased; pointer-events: auto; ${shape} }
.n:not(:popover-open) { display: none; }
.n.in { animation: escIn 260ms ease-out; }
.n.gone { animation: escOut 220ms ease-in forwards; }
.i { flex: none; width: 18px; height: 18px; color: ${css(p.icon)}; fill: currentColor; shape-rendering: crispEdges; }
.i.ln { fill: none; stroke: currentColor; stroke-width: 1.6px; stroke-linecap: round; stroke-linejoin: round; shape-rendering: geometricPrecision; }
.tt { flex: 1; min-width: 0; }
kbd { font: 500 .86em/1 ${MONO}; padding: .2em .42em; border-radius: ${red ? 0 : 4}px; white-space: nowrap;
  background: color-mix(in srgb, ${css(p.fg)} 15%, transparent); }
button { all: unset; flex: none; cursor: pointer; }
.act { font: 700 .86em/1 ${FONT}; color: ${css(p.actFg)}; background: ${css(p.actBg)}; border-radius: 7px; padding: .45em .7em; }
.act:hover { filter: brightness(1.2); }
.act:active { translate: 0 1px; }
.x { display: inline-grid; place-items: center; padding: .3em; border-radius: 5px; color: ${css(p.quiet)}; }
.x:hover { color: ${css(p.fg)}; background: color-mix(in srgb, ${css(p.fg)} 10%, transparent); }
.x .i { width: 9px; height: 9px; color: inherit; }
.x .i.ln { width: 12px; height: 12px; }
button:focus-visible { outline: 2px solid ${css(p.mark)}; outline-offset: 2px; }
.life { position: absolute; left: .8em; right: .8em; bottom: ${red ? 0 : 3}px; height: 2px; border-radius: 1px; background: ${css(p.mark)};
  opacity: .6; transform-origin: left; pointer-events: none; animation: escLife ${SHOW_MS}ms linear forwards; }
.n:is(:hover, :focus-within) .life { animation-play-state: paused; }
@keyframes escIn { from { transform: translateX(1.4em); opacity: 0; } }
@keyframes escOut { to { transform: translateX(1.4em); opacity: 0; } }
@keyframes escLife { to { transform: scaleX(0); } }`;
}

const SVG = "http://www.w3.org/2000/svg";
function icon(name: "undo" | "x"): SVGSVGElement {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("aria-hidden", "true");
  const line = themeState().icons === "line" ? LINE_ICONS[name] : undefined;
  if (line) {
    svg.setAttribute("class", "i ln");
    svg.setAttribute("viewBox", "0 0 24 24");
    const path = document.createElementNS(SVG, "path");
    path.setAttribute("d", line);
    path.setAttribute("vector-effect", "non-scaling-stroke");
    svg.append(path);
  } else {
    svg.setAttribute("class", "i");
    svg.setAttribute("viewBox", "0 0 9 9");
    for (const [x, y, w] of iconRects(name)) {
      const r = document.createElementNS(SVG, "rect");
      for (const [k, v] of [["x", x], ["y", y], ["width", w], ["height", 1]] as const) r.setAttribute(k, String(v));
      svg.append(r);
    }
  }
  return svg;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text) e.textContent = text;
  return e;
}

// the host's own inline styles, for a browser without the shadow sheet's :host rule
const HOST = "display:block!important;position:fixed!important;inset:0 auto auto 0!important;width:0!important;height:0!important;"
  + "visibility:visible!important;opacity:1!important;content-visibility:visible!important;zoom:1!important;pointer-events:none!important";
// what a theme could set on <html> that would take the top layer with it
const GUARD: Array<[string, string]> = [["display", "block"], ["content-visibility", "visible"]];

let host: HTMLElement | null = null;
let timer = 0;
let saved: Array<[string, string, string]> = [];

function show() {
  const to = previousTheme();
  if (!to) return;
  hide();
  const fs = textSize();
  const p = palette();
  const root = document.documentElement;

  const h = el("pm-escape" as "div", "");
  h.setAttribute("style", HOST);
  const sh = h.attachShadow({ mode: "open" });
  const style = el("style", "");
  style.textContent = sheet(p, fs);
  const n = el("div", `n${themeState().motion ? " in" : ""}`);
  n.setAttribute("popover", "manual");
  n.setAttribute("role", "status");
  const pad = [...(navigator.getGamepads?.() ?? [])].some((g) => g?.connected);
  const tt = el("span", "tt");
  tt.append("If this theme breaks anything, ", el("kbd", "", ESCAPE_KEY));
  if (pad) tt.append(" or ", el("kbd", "", "L3+R3"));
  tt.append(` goes back to ${to.name}`);
  const act = el("button", "act", "Go back");
  act.type = "button";
  act.title = `Go back to ${to.name} (${ESCAPE_KEY})`;
  act.addEventListener("click", themeBack);
  const x = el("button", "x");
  x.type = "button";
  x.title = "Dismiss";
  x.setAttribute("aria-label", "Dismiss");
  x.append(icon("x"));
  x.addEventListener("click", () => hide(true));
  const life = el("i", "life");
  life.setAttribute("aria-hidden", "true");
  n.append(icon("undo"), tt, act, x, life);
  sh.append(style, n);

  // where the toasts are: bottom right, left of the Nexus pane when it is open
  const t = document.querySelector(".toasts")?.getBoundingClientRect();
  const right = t && t.right > innerWidth * 0.3 && t.right <= innerWidth ? innerWidth - t.right : fs * 1.2;
  n.style.right = `${right}px`;
  n.style.bottom = `${fs * 1.2}px`;

  saved = GUARD.map(([k, v]) => { const was: [string, string, string] = [k, root.style.getPropertyValue(k), root.style.getPropertyPriority(k)]; root.style.setProperty(k, v, "important"); return was; });
  root.append(h);
  host = h;
  try { n.showPopover(); } catch { /* no top layer: it still shows, fixed above the page */ }
  // a theme that moved the toasts can't take the notice off the screen
  const r = n.getBoundingClientRect();
  if (r.left < 0 || r.right > innerWidth) n.style.right = `${fs * 1.2}px`;
  // a zoom on <html> reaches into the top layer: undo it
  const z = n.getBoundingClientRect().width / (n.offsetWidth || 1);
  if (Number.isFinite(z) && z > 0 && Math.abs(z - 1) > 0.02) n.style.zoom = String(1 / z);
  // toasts stack above it
  root.style.setProperty("--escape-h", `${Math.ceil(n.getBoundingClientRect().height + fs * 0.5)}px`);

  const arm = (ms: number) => { clearTimeout(timer); timer = window.setTimeout(() => hide(true), ms); };
  const hold = () => clearTimeout(timer);
  n.addEventListener("pointerenter", hold);
  n.addEventListener("focusin", hold);
  n.addEventListener("pointerleave", () => { if (!n.matches(":focus-within")) arm(2400); });
  n.addEventListener("focusout", () => { if (!n.matches(":hover")) arm(2400); });
  arm(SHOW_MS);
}

function hide(animate = false) {
  clearTimeout(timer);
  const h = host;
  if (!h) return;
  host = null;
  const root = document.documentElement;
  for (const [k, v, pr] of saved) { if (v) root.style.setProperty(k, v, pr); else root.style.removeProperty(k); }
  saved = [];
  root.style.removeProperty("--escape-h");
  if (!root.getAttribute("style")) root.removeAttribute("style");
  const n = h.shadowRoot?.querySelector<HTMLElement>(".n");
  if (!animate || !n || !themeState().motion) { h.remove(); return; }
  n.classList.add("gone");
  window.setTimeout(() => h.remove(), 230);
}
