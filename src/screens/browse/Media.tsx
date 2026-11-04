// Media: the community's best screenshots and videos, and a viewer for any list of images and videos.
import { useEffect, useState } from "react";
import { Icon } from "../../ui/Icon";
import { Empty, Key, Seg, formatCount } from "../../ui/pocket";
import { errorText, media, type MediaItem } from "../../lib/api";

export type Shot = { kind: "image" | "video"; url: string; thumb: string; caption?: string };

/** Full-window viewer: arrow keys or the keys at the sides move, Escape closes. Videos play embedded. */
export function Lightbox({ items, start, onClose }: { items: Shot[]; start: number; onClose: () => void }) {
  const [i, setI] = useState(start);
  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowRight") setI((x) => (x + 1) % items.length);
      if (e.key === "ArrowLeft") setI((x) => (x - 1 + items.length) % items.length);
    };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [items.length, onClose]);
  const it = items[i];
  return (
    <div className="lightbox" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="lb-stage">
        {it.kind === "video"
          ? <iframe src={it.url} title={it.caption ?? "Video"} allow="autoplay; encrypted-media; picture-in-picture" allowFullScreen />
          : <img src={it.url} alt={it.caption ?? ""} />}
      </div>
      <div className="lb-bar">
        {items.length > 1 && <Key size="sm" variant="dark" icon="chevron" className="flip" onClick={() => setI((i - 1 + items.length) % items.length)} aria-label="Previous" />}
        <span className="display sm inline"><span className="d-num">{String(i + 1).padStart(2, "0")}<small>/{items.length}</small></span></span>
        {it.caption && <span className="lb-cap">{it.caption}</span>}
        {items.length > 1 && <Key size="sm" variant="dark" icon="chevron" onClick={() => setI((i + 1) % items.length)} aria-label="Next" />}
        <Key size="sm" variant="dark" icon="x" onClick={onClose} aria-label="Close" />
      </div>
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
