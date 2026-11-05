// The CRT and glass layer (styles/fx.css): the overlay with scanlines and the vignette, the SVG filter behind the
// warped glass, and the tube's moments. Power on and off, and the channel change between views, are Collegiapp's
// (its fx.js powerOn, powerOff and channel, ported as they are): a tube TV turning on and off, with its snow, hum bar,
// degauss blotches, beam line and spot. A degauss after the theme changes is a class on <html> (fx.css). With
// motion off they are skipped, and with the CRT layer off the picture only fades in and out.

const root = () => document.documentElement;

/** The motion multiplier (pocket-os.css --m): 0 when motion is off or reduced. */
function motionM(): number {
  const v = parseFloat(getComputedStyle(root()).getPropertyValue("--m"));
  return Number.isFinite(v) ? v : 1;
}

const level = () => root().dataset.fx ?? "off";
/** The tube plays with the CRT layer on; without it the picture fades, as Collegiapp's Standard look does. */
const tube = () => level() !== "off";
const timers: Record<string, number> = {};

/** Put a moment's class on <html> for `ms` (scaled by the motion multiplier, as the CSS is). */
function moment(cls: "fx-dg" | "fx-chan", ms: number) {
  if (!tube() || !motionM() || document.visibilityState === "hidden" || run) return;
  const r = root();
  r.classList.remove(cls);
  void r.offsetWidth; // restart the animation
  r.classList.add(cls);
  clearTimeout(timers[cls]);
  timers[cls] = window.setTimeout(() => r.classList.remove(cls), ms + 60);
}

export const degauss = () => moment("fx-dg", 680);

// ── power: a tube TV turning on and off ──
// A bright line opens into a raster of snow (no signal yet) while the degauss coil swirls purity blotches through it;
// the picture rolls in, the horizontal hold tears and settles, the static fades as the signal locks, and colour comes
// on last. The app's own start (modules settling in, LCDs switching on) is held until BOOT_AT, so it plays through the
// last of the static. Off is the zip: the picture blooms, collapses to a bright line, the line shrinks to a dot and
// the dot fades on the phosphor; the window goes while the dot is still fading (GONE). Times are ms at full motion.

const ON = 760, OFF = 820, GONE = 660, BOOT_AT = 380;
type Run = { anims: Animation[]; timers: number[]; raf: number };
let run: Run | null = null;
const flick = "steps(1, end)";
const shell = (): HTMLElement[] => { const s = document.querySelector<HTMLElement>(".shell"); return s ? [s] : []; };

interface Layer { el: HTMLElement; snow: HTMLCanvasElement; hum: HTMLElement; veil: HTMLElement; tint: HTMLElement; glow: HTMLElement; line: HTMLElement; dot: HTMLElement }
function layer(): Layer {
  let el = document.querySelector<HTMLElement>("body > .tube");
  if (!el) {
    el = document.createElement("div");
    el.className = "tube";
    el.setAttribute("aria-hidden", "true");
    el.innerHTML = '<canvas class="crt-snow"></canvas><i class="crt-hum"></i><i class="crt-veil"></i><i class="crt-tint"></i><i class="crt-glow"></i><i class="crt-line"></i><i class="crt-dot"></i>';
    document.body.appendChild(el);
  }
  const q = <T extends Element = HTMLElement>(c: string) => el!.querySelector(".crt-" + c) as unknown as T;
  return { el, snow: q<HTMLCanvasElement>("snow"), hum: q("hum"), veil: q("veil"), tint: q("tint"), glow: q("glow"), line: q("line"), dot: q("dot") };
}

function stop() {
  blipStop();
  if (!run) return;
  const r = run; run = null;
  cancelAnimationFrame(r.raf); r.timers.forEach(clearTimeout); r.anims.forEach((a) => a.cancel());
  root().classList.remove("crt-run", "crt-hold");
}

type Frame = [number, Keyframe, string?];
// keyframes as [ms, props, easing to the next]; the offsets come from the times
const kf = (total: number, list: Frame[]): Keyframe[] => list.map(([t, p, e]) => ({ offset: Math.min(1, t / total), ...p, ...(e ? { easing: e } : {}) }));
function play(el: Element, total: number, list: Frame[], m: number) {
  const a = el.animate(kf(total, list), { duration: total * m, fill: "both" });
  run!.anims.push(a);
  return a;
}
const later = (ms: number, fn: () => void) => { run!.timers.push(window.setTimeout(fn, ms)); };
const bf = (b: number, s: number, c: number, bl: number): Keyframe => {
  const v = `brightness(${b}) saturate(${s}) contrast(${c}) blur(${bl}px)`;
  return { backdropFilter: v, webkitBackdropFilter: v } as Keyframe;
};

// Snow: a bandwidth-limited picture of nothing, one row per scanline (3 px), smeared sideways like a tuner's. A strip
// twice the screen's height is made once per size; each frame shows a random window of it.
let snowSrc: HTMLCanvasElement | null = null;
function snowSource(w: number, h: number): HTMLCanvasElement {
  if (snowSrc && snowSrc.width === w && snowSrc.height === h * 2) return snowSrc;
  const c = document.createElement("canvas"); c.width = w; c.height = h * 2;
  const g = c.getContext("2d")!, img = g.createImageData(w, h * 2), px = new Uint32Array(img.data.buffer);
  for (let y = 0; y < h * 2; y++) {
    let p = Math.random();
    const row = 0.82 + Math.random() * 0.36; // each line a little brighter or darker
    for (let x = 0; x < w; x++) {
      p = p * 0.42 + Math.random() * 0.58;
      const v = Math.random() > 0.993 ? 1 : Math.pow(p, 1.45) * row; // with sparse bright specks
      const b = Math.max(0, Math.min(255, v * 255)) | 0;
      px[y * w + x] = 0xff000000 | (b << 16) | (b << 8) | b;
    }
  }
  g.putImageData(img, 0, 0);
  return (snowSrc = c);
}
function snowLoop(cv: HTMLCanvasElement, m: number) {
  const w = Math.max(64, Math.ceil(window.innerWidth / 2)), h = Math.max(48, Math.ceil(window.innerHeight / 3));
  cv.width = w; cv.height = h;
  const g = cv.getContext("2d")!, src = snowSource(w, h), t0 = performance.now(), r = run!;
  const frame = (now: number) => {
    if (run !== r) return;
    const t = (now - t0) / m, flip = Math.random() < 0.5;
    g.setTransform(flip ? -1 : 1, 0, 0, 1, 0, 0);
    g.drawImage(src, 0, (Math.random() * h) | 0, w, h, flip ? -w : 0, 0, w, h);
    g.setTransform(1, 0, 0, 1, 0, 0);
    // the horizontal hold tears while the set hunts for sync: bands slide sideways, most while the picture rolls in
    const tear = t > 280 && t < 680 ? 0.55 : 0.12;
    for (let i = 0; i < 3; i++) if (Math.random() < tear) {
      const y = (Math.random() * h) | 0, bh = 1 + ((Math.random() * h * 0.12) | 0), dx = (Math.random() - 0.5) * w * 0.08;
      g.drawImage(cv, 0, y, w, bh, dx, y, w, bh);
    }
    if (t < ON + 80) r.raf = requestAnimationFrame(frame);
  };
  r.raf = requestAnimationFrame(frame);
}

/** Turn the picture on. */
export function powerOn(done?: () => void) {
  stop();
  const r = root(), m = motionM();
  r.classList.remove("off");
  if (!m || document.visibilityState === "hidden") { r.classList.remove("crt-hold"); done?.(); return; }
  run = { anims: [], timers: [], raf: 0 };
  if (!tube()) { // no CRT layer: the picture fades up
    shell().forEach((el) => play(el, 220, [[0, { opacity: 0 }, "ease-out"], [220, { opacity: 1 }]], m));
    later(220 * m + 30, () => { stop(); done?.(); });
    return;
  }
  r.classList.add("crt-run");
  const L = layer(), open = "cubic-bezier(.2, .8, .3, 1)";
  // the raster: a line opening top and bottom, blooming a little past full height; then the picture rolls in from
  // above and the hold jitters before it locks
  shell().forEach((el) => play(el, ON, [
    [0, { transform: "translate(0, 0) scale(0, .004)", opacity: 0 }],
    [100, { transform: "translate(0, 0) scale(1, .004)", opacity: 0 }, open],
    [230, { transform: "translate(0, 0) scale(1, 1.03)", opacity: 0 }, "ease-out"],
    [270, { transform: "translate(0, 0) scale(1, 1)", opacity: 0 }],
    [360, { transform: "translate(0, -40%) scale(1, 1)", opacity: 0.14 }, "cubic-bezier(.3, .6, .4, 1)"],
    [430, { transform: "translate(0, -7%) scale(1, 1)", opacity: 0.36 }, "ease-out"],
    [460, { transform: "translate(0, 3.5%) scale(1, 1)", opacity: 0.45 }, "ease-in-out"],
    [490, { transform: "translate(5px, -1.5%) scale(1, 1)", opacity: 0.55 }, flick],
    [515, { transform: "translate(-4px, .6%) scale(1, 1)", opacity: 0.62 }, flick],
    [545, { transform: "translate(3px, 0) scale(1, 1)", opacity: 0.7 }, flick],
    [575, { transform: "translate(-1.5px, 0) scale(1, 1)", opacity: 0.78 }, flick],
    [605, { transform: "translate(.5px, 0) scale(1, 1)", opacity: 0.86 }, flick],
    [635, { transform: "translate(0, 0) scale(1, 1)", opacity: 0.93 }, "ease-out"],
    [ON, { transform: "translate(0, 0) scale(1, 1)", opacity: 1 }],
  ], m));
  play(L.snow, ON, [
    [0, { transform: "scale(0, .004)", opacity: 0 }], [100, { transform: "scale(1, .004)", opacity: 1 }, open],
    [230, { transform: "scale(1, 1.03)", opacity: 1 }, "ease-out"], [270, { transform: "scale(1, 1)", opacity: 1 }],
    [400, { transform: "scale(1, 1)", opacity: 0.9 }, "cubic-bezier(.4, 0, .6, 1)"], [600, { transform: "scale(1, 1)", opacity: 0.3 }, "ease-out"],
    [ON, { transform: "scale(1, 1)", opacity: 0 }],
  ], m);
  // the hum bar: a darker band drifting up through the snow (the supply's ripple beating against the field rate)
  play(L.hum, ON, [[0, { transform: "translateY(380%)", opacity: 0 }, "linear"], [180, { transform: "translateY(280%)", opacity: 1 }, "linear"],
    [520, { transform: "translateY(60%)", opacity: 0.8 }, "linear"], [ON, { transform: "translateY(-120%)", opacity: 0 }]], m);
  // the beam's centre line before the raster opens
  play(L.line, ON, [[0, { transform: "scale(0, 1)", opacity: 0 }], [25, { transform: "scale(.05, 1)", opacity: 1 }, "cubic-bezier(.3, 0, .2, 1)"],
    [95, { transform: "scale(1.02, 1)", opacity: 1 }], [120, { transform: "scale(1.02, 1.4)", opacity: 1 }, "ease-in"],
    [240, { transform: "scale(1.02, 14)", opacity: 0 }], [ON, { transform: "scale(1.02, 14)", opacity: 0 }]], m);
  // overdriven while the tube warms, focus soft, and black and white until the colour burst locks (a touch too rich,
  // then right), so the first modules settle in grey and take their colour as they land
  play(L.veil, ON, [[0, bf(2.6, 0, 1.25, 1.4)], [100, bf(2.6, 0, 1.25, 1.4), "ease-out"], [270, bf(1.6, 0, 1.15, 1.1)], [440, bf(1.25, 0, 1.1, 0.7)],
    [520, bf(1.2, 0.15, 1.06, 0.3), "ease-out"], [600, bf(1.1, 1.35, 1.03, 0)], [680, bf(1.04, 1.1, 1.01, 0)], [ON, bf(1, 1, 1, 0)]], m);
  // the degauss coil: purity blotches swirl and die away with its hum
  play(L.tint, ON, [[0, { transform: "rotate(0deg) scale(1.25)", opacity: 0 }], [80, { transform: "rotate(0deg) scale(1.25)", opacity: 0 }, "ease-out"],
    [160, { transform: "rotate(-6deg) scale(1.2)", opacity: 0.85 }], [260, { transform: "rotate(16deg) scale(1.12)", opacity: 0.72 }],
    [370, { transform: "rotate(-11deg) scale(1.16)", opacity: 0.55 }], [470, { transform: "rotate(7deg) scale(1.1)", opacity: 0.4 }],
    [560, { transform: "rotate(-4deg) scale(1.13)", opacity: 0.24 }], [640, { transform: "rotate(1.5deg) scale(1.1)", opacity: 0.1 }],
    [720, { transform: "rotate(0deg) scale(1.1)", opacity: 0 }]], m);
  snowLoop(L.snow, m);
  // the app starts as the static clears: its modules settle in and its LCDs switch on through the last of the snow
  later(BOOT_AT * m, () => r.classList.remove("crt-hold"));
  later(ON * m + 40, () => { stop(); done?.(); });
}

/** Turn the picture off, then run `done` (which closes the window) while the dot is still fading. */
export function powerOff(done?: () => void) {
  stop();
  // native layers (the browser pane) do not take part in the animation: they step aside first
  window.dispatchEvent(new Event("pm-power-off"));
  const r = root(), m = motionM();
  let handed = false;
  const gone = () => { if (!handed) { handed = true; done?.(); } };
  const end = () => { r.classList.add("off"); stop(); };
  if (!m || r.classList.contains("off") || document.visibilityState === "hidden") { end(); gone(); return; }
  run = { anims: [], timers: [], raf: 0 };
  if (!tube()) { // no CRT layer: the picture fades out
    shell().forEach((el) => play(el, 160, [[0, { opacity: 1 }, "ease-in"], [160, { opacity: 0 }]], m));
    later(160 * m + 20, () => { end(); gone(); });
    return;
  }
  r.classList.add("crt-run");
  const L = layer(), collapse = "cubic-bezier(.62, 0, .88, .42)";
  // the picture blooms as the high voltage sags, then the vertical deflection lets go
  shell().forEach((el) => play(el, OFF, [
    [0, { transform: "scale(1, 1)", opacity: 1 }, "ease-out"], [55, { transform: "scale(1.012, .985)", opacity: 1 }, collapse],
    [250, { transform: "scale(1.035, .006)", opacity: 1 }, "linear"], [266, { transform: "scale(1.035, .006)", opacity: 0 }],
    [OFF, { transform: "scale(1.035, .006)", opacity: 0 }],
  ], m));
  play(L.veil, OFF, [[0, bf(1, 1, 1, 0), "ease-out"], [55, bf(1.25, 0.95, 1.05, 0), "ease-in"], [250, bf(3.2, 0.35, 1.2, 0.7)], [OFF, bf(3.2, 0.35, 1.2, 0.7)]], m);
  // the bright line, held a moment, then the horizontal lets go too and it zips to the centre
  play(L.line, OFF, [[0, { transform: "scale(1.035, 1)", opacity: 0 }], [228, { transform: "scale(1.035, 1)", opacity: 0 }, "ease-out"],
    [252, { transform: "scale(1.035, 1.8)", opacity: 1 }, "ease-out"], [300, { transform: "scale(1.035, 1)", opacity: 1 }, flick],
    [318, { transform: "scale(1.035, 1)", opacity: 0.82 }, flick], [334, { transform: "scale(1.035, 1)", opacity: 1 }, "cubic-bezier(.55, 0, .85, .35)"],
    [515, { transform: "scale(.004, .8)", opacity: 1 }, "linear"], [540, { transform: "scale(.004, .8)", opacity: 0 }], [OFF, { transform: "scale(.004, .8)", opacity: 0 }]], m);
  // the spot: the beam with nothing deflecting it, fading on the phosphor (the window goes partway through the fade)
  play(L.dot, OFF, [[0, { transform: "scale(.3)", opacity: 0 }], [480, { transform: "scale(.3)", opacity: 0 }, "ease-out"], [522, { transform: "scale(1.15)", opacity: 1 }, "ease-in-out"],
    [600, { transform: "scale(1)", opacity: 1 }, "cubic-bezier(.2, .6, .4, 1)"], [OFF, { transform: "scale(.5)", opacity: 0 }]], m);
  // the phosphor's afterglow of the picture that was there
  play(L.glow, OFF, [[0, { opacity: 0 }, "ease-out"], [250, { opacity: 0.24 }, "cubic-bezier(.2, .6, .4, 1)"], [OFF, { opacity: 0 }]], m);
  later(GONE * m, gone);
  later(OFF * m + 30, end);
}

export const powered = () => !root().classList.contains("off");

// ── between views: Collegiapp's channel change ──
// Only the part that changes is retuned, and its brightness never jumps: the new picture comes in a touch soft and
// muted under faint static (blended soft-light, so it textures the picture instead of lighting it), then sharpens and
// settles as the colour locks, as it swings up a few pixels into place. Nothing waits for it, a new one replaces the
// last, and none plays during power, with motion off or with the CRT layer off. The view never grows a scrollbar for
// it: the shell clips the swing (fx.css).
let blipping: Run | null = null;
function blipStop() {
  if (!blipping) return;
  const b = blipping; blipping = null;
  cancelAnimationFrame(b.raf); b.timers.forEach(clearTimeout); b.anims.forEach((a) => a.cancel());
  root().classList.remove("crt-blip");
  const cv = document.querySelector<HTMLCanvasElement>("body > .tube .crt-snow");
  if (cv) { cv.removeAttribute("style"); cv.classList.remove("soft"); }
}

let first = true;
/** A channel change on the view that just rendered; the first call (the view the app opens on) is covered by power on. */
export function channel() {
  if (first) { first = false; return; }
  const box = document.querySelector<HTMLElement>(".shell > .view");
  const m = motionM();
  if (!box || !m || run || root().classList.contains("off") || document.visibilityState === "hidden") return;
  if (level() !== "full") { moment("fx-chan", 300); return; } // subtle: a brightness blip (fx.css)
  const T = 440, TOTAL = 1150;
  blipStop();
  const b: Run = (blipping = { anims: [], timers: [], raf: 0 }), L = layer();
  root().classList.add("crt-blip");
  b.timers.push(window.setTimeout(blipStop, TOTAL * m + 40));
  const anim = (el: Element, t: number, list: Frame[]) => { const a = el.animate(kf(t, list), { duration: t * m, fill: "both" }); b.anims.push(a); return a; };
  const r = box.getBoundingClientRect();
  const w = Math.max(64, Math.ceil(r.width / 2)), h = Math.max(48, Math.ceil(r.height / 3));
  L.snow.width = w; L.snow.height = h;
  const g = L.snow.getContext("2d")!, src = snowSource(w, h);
  Object.assign(L.snow.style, { top: r.top + "px", left: r.left + "px", width: r.width + "px", height: r.height + "px", right: "auto", bottom: "auto" });
  L.snow.classList.add("soft");
  // the snow is faint, eases in rather than popping up, and is redrawn at about 20 fps, so it grains the picture
  // without strobing
  const t0 = performance.now(), SNOW = 240 + Math.random() * 40;
  let drawn = -Infinity;
  const frame = (now: number) => {
    if (blipping !== b) return;
    if (now - drawn >= 48) { drawn = now; g.drawImage(src, 0, (Math.random() * h) | 0, w, h, 0, 0, w, h); }
    if (now - t0 < SNOW * m) b.raf = requestAnimationFrame(frame);
  };
  frame(t0); // drawn now, so it is up in the same frame as the new view
  anim(L.snow, SNOW, [[0, { opacity: 0.08 }, "ease-out"], [SNOW * 0.2, { opacity: 0.2 }, "ease-in-out"], [SNOW, { opacity: 0 }]]);
  const rise = `translateY(${(0.3 + Math.random() * 0.35).toFixed(2)}em)`; // a little different every time
  anim(box, T, [[0, { transform: rise, filter: "blur(.6px) saturate(.6) contrast(1.02)" }, "cubic-bezier(.2, .7, .3, 1)"],
    [T * 0.5, { transform: "translateY(-1px)", filter: "blur(.2px) saturate(.9) contrast(1.01)" }, "ease-out"],
    [T, { transform: "none", filter: "blur(0px) saturate(1) contrast(1)" }]]);
}

const SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0" style="position:absolute" aria-hidden="true" focusable="false">
  <filter id="pm-glass" x="-5%" y="-20%" width="110%" height="140%" color-interpolation-filters="sRGB">
    <feTurbulence type="fractalNoise" baseFrequency="0.006 0.035" numOctaves="2" seed="7" result="n"/>
    <feGaussianBlur in="n" stdDeviation="1.5" result="s"/>
    <feDisplacementMap in="SourceGraphic" in2="s" scale="14" xChannelSelector="R" yChannelSelector="G"/>
  </filter>
</svg>`;

/** Add the overlay and the glass filter, and power on. Call once at start, before the app first renders. */
export function installFx() {
  if (document.querySelector("body > .crt")) return;
  const crt = document.createElement("div");
  crt.className = "crt";
  crt.setAttribute("aria-hidden", "true");
  crt.innerHTML = '<i class="scan"></i><i class="vig"></i>';
  document.body.appendChild(crt);
  const defs = document.createElement("div");
  defs.innerHTML = SVG;
  const svg = defs.firstElementChild;
  if (svg) document.body.appendChild(svg);
  window.addEventListener("pm-theme", (e) => { if ((e as CustomEvent).detail === "theme" || (e as CustomEvent).detail === "fx") degauss(); });
  // dark until the tube comes on, and the app's own start held for it, so its first frame is never seen before
  if (motionM()) root().classList.add("off", "crt-hold");
  // React renders a frame or two later: wait for the shell, so the picture has something to open on
  const on = (tries = 0) => { requestAnimationFrame(() => { if (!document.querySelector(".shell") && tries < 120) on(tries + 1); else powerOn(); }); };
  if (document.visibilityState === "hidden") {
    const wake = () => { if (document.visibilityState !== "hidden") { document.removeEventListener("visibilitychange", wake); on(); } };
    document.addEventListener("visibilitychange", wake);
  } else on();
}
