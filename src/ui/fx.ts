// The CRT and glass layer (styles/fx.css): the overlay with scanlines and the vignette, the SVG filter behind the
// warped glass, and the tube's three moments: power on at start, a channel change when the view changes, a degauss
// after the theme changes. Each moment is a class on <html> for as long as its animation runs; with motion off they
// are skipped. Nothing here changes what is on screen beyond those classes.

const root = () => document.documentElement;

function motionOn(): boolean {
  const m = root().dataset.motion;
  if (m === "0") return false;
  if (m === "1") return true;
  return !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

const level = () => root().dataset.fx ?? "off";
const timers: Record<string, number> = {};

/** Put a moment's class on <html> for `ms` (scaled by the motion multiplier, as the CSS is). */
function moment(cls: "fx-power" | "fx-chan" | "fx-dg", ms: number) {
  if (level() === "off" || !motionOn() || document.visibilityState === "hidden") return;
  if (cls === "fx-chan" && root().classList.contains("fx-power")) return; // still powering on
  const r = root();
  r.classList.remove(cls);
  void r.offsetWidth; // restart the animation
  r.classList.add(cls);
  clearTimeout(timers[cls]);
  timers[cls] = window.setTimeout(() => r.classList.remove(cls), ms + 60);
}

export const powerOn = () => moment("fx-power", 760);
export const degauss = () => moment("fx-dg", 680);
let first = true;
/** A channel change; the first call (the view the app opens on) is covered by power on. */
export function channel() {
  if (first) { first = false; return; }
  moment("fx-chan", 300);
}

const SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0" style="position:absolute" aria-hidden="true" focusable="false">
  <filter id="pm-glass" x="-5%" y="-20%" width="110%" height="140%" color-interpolation-filters="sRGB">
    <feTurbulence type="fractalNoise" baseFrequency="0.006 0.035" numOctaves="2" seed="7" result="n"/>
    <feGaussianBlur in="n" stdDeviation="1.5" result="s"/>
    <feDisplacementMap in="SourceGraphic" in2="s" scale="14" xChannelSelector="R" yChannelSelector="G"/>
  </filter>
</svg>`;

/** Add the overlay and the glass filter, and power on. Call once at start. */
export function installFx() {
  if (document.querySelector(".crt")) return;
  const crt = document.createElement("div");
  crt.className = "crt";
  crt.setAttribute("aria-hidden", "true");
  crt.innerHTML = '<i class="scan"></i><i class="vig"></i><i class="snow"></i><i class="beam"></i>';
  document.body.appendChild(crt);
  const defs = document.createElement("div");
  defs.innerHTML = SVG;
  const svg = defs.firstElementChild;
  if (svg) document.body.appendChild(svg);
  window.addEventListener("pm-theme", (e) => { if ((e as CustomEvent).detail === "theme" || (e as CustomEvent).detail === "fx") degauss(); });
  requestAnimationFrame(() => powerOn());
}
