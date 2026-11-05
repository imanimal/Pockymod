// Media: the community's best screenshots and videos, and a viewer for any list of images and videos.
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Icon } from "../../ui/Icon";
import { Empty, Key, Seg, formatCount } from "../../ui/pocket";
import { errorText, media, openExternal, type MediaItem } from "../../lib/api";
import { toast } from "../../ui/toast";

export type Shot = { kind: "image" | "video"; url: string; thumb: string; caption?: string };

/** Full-window viewer, over everything in the window (never inside the drawer that opened it). A picture always
 *  fits the window until you zoom: the wheel zooms where the pointer is, drag moves a zoomed picture, double-click
 *  goes between fit and close up, and + - 0 1 do the same from the keyboard. Arrow keys or the keys at the sides
 *  move between items, Escape closes. Videos play embedded. */
/** A YouTube video's own page, for the user's browser. */
export const watchUrl = (embed: string) => { const id = embed.match(/embed\/([A-Za-z0-9_-]{11})/)?.[1]; return id ? `https://www.youtube.com/watch?v=${id}` : null; };
const watch = (it: Shot) => { const u = watchUrl(it.url); if (u) void openExternal(u).catch((e) => toast(errorText(e), true)); };
const VIDEOS = "pm.videos.here";

export function Lightbox({ items, start, onClose }: { items: Shot[]; start: number; onClose: () => void }) {
  const [i, setI] = useState(start);
  // videos open in the user's browser by default: an embedded YouTube player can ask to sign in to prove you are
  // not a bot, and Google refuses sign-ins inside apps. "Always play videos here" keeps them in the app.
  const [here, setHereState] = useState(() => { try { return localStorage.getItem(VIDEOS) === "1"; } catch { return false; } });
  const setHere = (v: boolean) => { setHereState(v); try { localStorage.setItem(VIDEOS, v ? "1" : "0"); } catch { /* storage unavailable */ } };
  const [playHere, setPlayHere] = useState(false);
  useEffect(() => setPlayHere(false), [i]);
  const zoom = useRef<ZoomControls | null>(null);
  const [z, setZ] = useState<{ pct: number; fit: boolean }>({ pct: 100, fit: true });
  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement | null)?.closest?.("input, textarea, select")) return;
      if (e.key === "Escape") { e.stopPropagation(); onClose(); }
      else if (e.key === "ArrowRight") setI((x) => (x + 1) % items.length);
      else if (e.key === "ArrowLeft") setI((x) => (x - 1 + items.length) % items.length);
      else if (e.key === "+" || e.key === "=") zoom.current?.step(1);
      else if (e.key === "-" || e.key === "_") zoom.current?.step(-1);
      else if (e.key === "0") zoom.current?.fit();
      else if (e.key === "1") zoom.current?.actual();
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", k, true);
    return () => window.removeEventListener("keydown", k, true);
  }, [items.length, onClose]);
  const it = items[i];
  const host = document.querySelector(".shell") ?? document.body;
  return createPortal(
    <div className="lightbox" role="dialog" aria-modal="true" aria-label={it.caption ?? "Media"}>
      {it.kind === "video"
        ? <div className="lb-stage video" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
            {here || playHere ? <iframe src={it.url} title={it.caption ?? "Video"} allow="autoplay; encrypted-media; picture-in-picture" allowFullScreen />
              : <div className="lb-video">
                  <img src={it.thumb.replace("/mqdefault.", "/hqdefault.")} alt="" />
                  <div className="lb-video-keys">
                    <Key variant="primary" size="lg" icon="play" onClick={() => watch(it)} title="Opens in your browser">Watch on YouTube</Key>
                    <Key variant="dark" onClick={() => setPlayHere(true)} title="YouTube may block some videos in the app">Play here</Key>
                  </div>
                  <label className="lb-video-pref"><input type="checkbox" checked={here} onChange={(e) => setHere(e.target.checked)} /> Always play videos here</label>
                </div>}
          </div>
        : <ZoomImage key={it.url} src={it.url} alt={it.caption ?? ""} controls={zoom} onChange={setZ} onBackdrop={onClose} />}
      <div className="lb-bar">
        {items.length > 1 && <Key size="sm" variant="dark" icon="chevron" className="flip" onClick={() => setI((i - 1 + items.length) % items.length)} aria-label="Previous" />}
        <span className="display sm inline"><span className="d-num">{String(i + 1).padStart(2, "0")}<small>/{items.length}</small></span></span>
        {items.length > 1 && <Key size="sm" variant="dark" icon="chevron" onClick={() => setI((i + 1) % items.length)} aria-label="Next" />}
        {it.caption && <span className="lb-cap">{it.caption}</span>}
        {it.kind === "image" && <span className="lb-zoom">
          <Key size="sm" variant="dark" icon="minus" onClick={() => zoom.current?.step(-1)} disabled={z.fit} aria-label="Zoom out" title="Zoom out (-)" />
          <span className="display sm inline" title={z.fit ? "Fits the window" : "Zoom, of the picture's own size"}><span className="d-lbl">{z.fit ? "Fit" : "Zoom"}</span><span className="d-num">{z.pct}<small>%</small></span></span>
          <Key size="sm" variant="dark" icon="plus" onClick={() => zoom.current?.step(1)} aria-label="Zoom in" title="Zoom in (+)" />
          <Key size="sm" variant="dark" onClick={() => zoom.current?.fit()} disabled={z.fit} title="Fit (0)">Fit</Key>
          <Key size="sm" variant="dark" onClick={() => zoom.current?.actual()} title="Actual size (1)">1:1</Key>
        </span>}
        {it.kind === "video" && <Key size="sm" variant="dark" icon="link" onClick={() => watch(it)} title="Watch in your browser">YouTube</Key>}
        <Key size="sm" variant="dark" icon="x" onClick={onClose} aria-label="Close" />
      </div>
    </div>, host);
}

interface ZoomControls { step: (dir: 1 | -1) => void; fit: () => void; actual: () => void }

/** A picture that fits its stage, zoomed and moved by wheel, drag, double-click and the controls. */
function ZoomImage({ src, alt, controls, onChange, onBackdrop }: {
  src: string; alt: string; controls: React.MutableRefObject<ZoomControls | null>; onChange: (z: { pct: number; fit: boolean }) => void; onBackdrop: () => void;
}) {
  const stage = useRef<HTMLDivElement>(null);
  const img = useRef<HTMLImageElement>(null);
  // z is relative to the fitted size (1 = fits); x, y move the picture's centre from the stage's centre
  const v = useRef({ z: 1, x: 0, y: 0 });
  const [, paint] = useState(0);
  const drag = useRef<{ px: number; py: number; x: number; y: number; moved: boolean } | null>(null);
  const [dragging, setDragging] = useState(false);
  // how many screen pixels one picture pixel is when it fits (1 for a picture smaller than the stage)
  const fitScale = () => { const i = img.current; return i && i.naturalWidth ? i.offsetWidth / i.naturalWidth : 1; };
  const maxZ = () => Math.max(2, 4 / fitScale());
  const report = () => onChange({ pct: Math.round(fitScale() * v.current.z * 100), fit: v.current.z <= 1.001 });
  const clamp = (z: number, x: number, y: number) => {
    const s = stage.current, i = img.current;
    if (!s || !i) return { z, x: 0, y: 0 };
    const mx = Math.max(0, (i.offsetWidth * z - s.clientWidth) / 2), my = Math.max(0, (i.offsetHeight * z - s.clientHeight) / 2);
    return { z, x: Math.max(-mx, Math.min(mx, x)), y: Math.max(-my, Math.min(my, y)) };
  };
  const set = (z: number, x: number, y: number) => { v.current = clamp(z, x, y); paint((n) => n + 1); report(); };
  /** Zoom to `nz`, keeping the point (cx, cy) from the stage's centre where it is. */
  const zoomAt = (nz: number, cx = 0, cy = 0) => {
    const c = v.current, z2 = Math.max(1, Math.min(maxZ(), nz)), k = z2 / c.z;
    set(z2, cx - (cx - c.x) * k, cy - (cy - c.y) * k);
  };
  const fromCentre = (e: { clientX: number; clientY: number }) => {
    const r = stage.current!.getBoundingClientRect();
    return [e.clientX - (r.left + r.width / 2), e.clientY - (r.top + r.height / 2)] as const;
  };
  controls.current = {
    step: (dir) => zoomAt(v.current.z * (dir > 0 ? 1.4 : 1 / 1.4)),
    fit: () => set(1, 0, 0),
    actual: () => zoomAt(1 / fitScale()),
  };
  useEffect(() => {
    const s = stage.current;
    if (!s) return;
    // the wheel zooms the picture, not the page behind it (so not a passive listener)
    const wheel = (e: WheelEvent) => { e.preventDefault(); const [cx, cy] = fromCentre(e); zoomAt(v.current.z * Math.pow(1.0018, -e.deltaY), cx, cy); };
    s.addEventListener("wheel", wheel, { passive: false });
    // a new window size: the picture fits again at the same zoom, kept inside the stage
    const ro = new ResizeObserver(() => { const c = v.current; v.current = clamp(c.z, c.x, c.y); paint((n) => n + 1); report(); });
    ro.observe(s);
    return () => { s.removeEventListener("wheel", wheel); ro.disconnect(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const c = v.current;
  return (
    <div ref={stage} className={`lb-stage${c.z > 1.001 ? " zoomed" : ""}${dragging ? " dragging" : ""}`}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        drag.current = { px: e.clientX, py: e.clientY, x: c.x, y: c.y, moved: false };
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      }}
      onPointerMove={(e) => {
        const d = drag.current;
        if (!d) return;
        const dx = e.clientX - d.px, dy = e.clientY - d.py;
        if (!d.moved && Math.hypot(dx, dy) < 4) return;
        d.moved = true; setDragging(true);
        if (v.current.z > 1.001) set(v.current.z, d.x + dx, d.y + dy);
      }}
      onPointerUp={(e) => {
        const d = drag.current; drag.current = null; setDragging(false);
        // a click on the dark around a fitted picture closes, as before
        if (d && !d.moved && e.target === e.currentTarget && v.current.z <= 1.001) onBackdrop();
      }}
      onDoubleClick={(e) => { const [cx, cy] = fromCentre(e); if (v.current.z > 1.001) set(1, 0, 0); else zoomAt(Math.max(2.5, 1 / fitScale()), cx, cy); }}>
      <img ref={img} className="lb-img" src={src} alt={alt} draggable={false} onLoad={report}
        style={{ transform: `translate(-50%, -50%) translate(${c.x}px, ${c.y}px) scale(${c.z})` }} />
    </div>
  );
}

/** A big picture with the strip under it, for a mod's or a collection's page: the arrows or a thumbnail change it,
 *  a click opens the viewer. */
export function MediaStage({ items }: { items: Shot[] }) {
  const [i, setI] = useState(0);
  const [open, setOpen] = useState<number | null>(null);
  if (items.length === 0) return null;
  const it = items[Math.min(i, items.length - 1)];
  return (
    <div className="mstage">
      <button type="button" className="mstage-pic" onClick={() => setOpen(i)} aria-label={it.kind === "video" ? "Play video" : "View picture, full window"}>
        <img src={it.kind === "video" ? it.thumb : it.url} alt="" />
        {it.kind === "video" && <span className="gplay"><Icon name="play" /></span>}
        <span className="mstage-hint"><Icon name="max" size={9} /> Full window</span>
      </button>
      {items.length > 1 && <>
        <Key size="sm" variant="dark" icon="chevron" className="flip mstage-prev" onClick={() => setI((i - 1 + items.length) % items.length)} aria-label="Previous picture" />
        <Key size="sm" variant="dark" icon="chevron" className="mstage-next" onClick={() => setI((i + 1) % items.length)} aria-label="Next picture" />
        <div className="gstrip">
          {items.map((m, n) => (
            <button key={m.url} type="button" className={`gthumb${n === i ? " on" : ""}`} onClick={() => setI(n)} aria-label={`${m.kind === "video" ? "Video" : "Picture"} ${n + 1}`} aria-pressed={n === i}>
              <img src={m.thumb} alt="" loading="lazy" />
              {m.kind === "video" && <span className="gplay"><Icon name="play" /></span>}
            </button>
          ))}
        </div>
      </>}
      {open !== null && <Lightbox items={items} start={open} onClose={() => setOpen(null)} />}
    </div>
  );
}

/** A strip of thumbnails that opens the viewer. */
export function Gallery({ items }: { items: Shot[] }) {
  const [open, setOpen] = useState<number | null>(null);
  if (items.length === 0) return null;
  return (
    <>
      <div className="gstrip">
        {items.map((m, n) => (
          <button key={m.url} type="button" className="gthumb" onClick={() => setOpen(n)} aria-label={m.kind === "video" ? "Play video" : "View picture"}>
            <img src={m.thumb} alt="" loading="lazy" />
            {m.kind === "video" && <span className="gplay"><Icon name="play" /></span>}
          </button>
        ))}
      </div>
      {open !== null && <Lightbox items={items} start={open} onClose={() => setOpen(null)} />}
    </>
  );
}

const toShot = (m: MediaItem): Shot => m.__typename === "Video"
  ? { kind: "video", url: `https://www.youtube-nocookie.com/embed/${(m.link ?? "").match(/(?:v=|youtu\.be\/|embed\/)([A-Za-z0-9_-]{11})/)?.[1] ?? ""}`, thumb: m.thumbnailUrl, caption: `${m.title ?? "Video"} · ${m.owner.name}` }
  : { kind: "image", url: m.url ?? m.thumbnailUrl, thumb: m.thumbnailUrl, caption: `${m.title || m.owner.name} · ${formatCount(m.views)} views` };

/** The game's media on Nexus: top-rated, most viewed or newest screenshots and videos. */
export function MediaWall() {
  const [kind, setKind] = useState<"image" | "video">("image");
  const [sort, setSort] = useState<"rating" | "views" | "createdAt">("rating");
  const [items, setItems] = useState<MediaItem[]>([]);
  const [total, setTotal] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const load = (offset: number) => {
    setBusy(true);
    media.browse(kind, sort, offset).then((p) => { setTotal(p.totalCount); setItems((old) => offset ? [...old, ...p.nodes] : p.nodes); setErr(null); })
      .catch((e) => setErr(errorText(e))).finally(() => setBusy(false));
  };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { setItems([]); load(0); }, [kind, sort]);
  const shots = items.map(toShot);
  return (
    <div className="stack">
      <div className="actions">
        <Seg label="Kind" value={kind} onChange={setKind} options={[{ value: "image", label: "Screenshots" }, { value: "video", label: "Videos" }]} />
        <Seg label="Sort" value={sort} onChange={setSort} options={[{ value: "rating", label: "Top rated" }, { value: "views", label: "Most viewed" }, { value: "createdAt", label: "Newest" }]} />
        <span className="dim mono" style={{ fontSize: ".8em" }}>{formatCount(total)} on Nexus</span>
      </div>
      {err && <div className="note bad">{err}</div>}
      {!busy && items.length === 0 && !err && <Empty icon="gallery" title="Nothing here yet" />}
      <div className="mwall">
        {shots.map((m, n) => (
          <button key={`${m.url}-${n}`} type="button" className="mtile" onClick={() => setOpen(n)}>
            <img src={m.thumb} alt="" loading="lazy" />
            {m.kind === "video" && <span className="gplay"><Icon name="play" /></span>}
            <span className="mtile-cap">{m.caption}</span>
          </button>
        ))}
      </div>
      {items.length < total && <div className="actions" style={{ justifyContent: "center" }}><Key icon="down" disabled={busy} onClick={() => load(items.length)}>Show more</Key></div>}
      {open !== null && <Lightbox items={shots} start={open} onClose={() => setOpen(null)} />}
    </div>
  );
}
