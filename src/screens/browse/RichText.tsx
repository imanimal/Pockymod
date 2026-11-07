// A description laid out the way its author wrote it: sizes, alignment, lists, tables, quotes, spoilers, links, and
// pictures and videos where they sit in the text. A click on a picture or video opens it in the viewer, with the
// description's other pictures and videos beside it.
import { useMemo, useState, type CSSProperties, type MouseEvent, type ReactNode } from "react";
import { Icon } from "../../ui/Icon";
import { parseRich, richMedia, type RichEl, type RichNode } from "../../lib/rich";
import { bareLinks } from "../../lib/text";
import { Lightbox, type Shot } from "./Media";
import { openLink, tipOf } from "./Links";

const toShot = (m: RichEl): Shot => m.k === "video"
  ? { kind: "video", url: `https://www.youtube-nocookie.com/embed/${m.v}`, thumb: `https://img.youtube.com/vi/${m.v}/mqdefault.jpg` }
  : { kind: "image", url: m.v!, thumb: m.v! };

export function RichText({ src, onMod }: { src: string; onMod?: (modId: number) => void }) {
  const nodes = useMemo(() => parseRich(src), [src]);
  const media = useMemo(() => richMedia(nodes), [nodes]);
  const [open, setOpen] = useState<number | null>(null);

  const link = (url: string, kids: ReactNode, key: number | string) => {
    const go = (e: MouseEvent) => { e.preventDefault(); openLink(url, onMod, e.ctrlKey || e.metaKey || e.shiftKey || e.button === 1); };
    return <a key={key} href={url} className="dlink" onClick={go} onAuxClick={(e) => { if (e.button === 1) go(e); }} title={tipOf(url)}>{kids}</a>;
  };
  const text = (t: string, key: number) => bareLinks(t).map((p, j) => typeof p === "string" ? p : link(p.url, p.text, `${key}-${j}`));

  const render = (ns: RichNode[], inLink = false): ReactNode[] => ns.map((n, i) => {
    if (typeof n === "string") return inLink ? n : <span key={i}>{text(n, i)}</span>;
    const kids = () => render(n.kids, inLink || n.k === "link");
    switch (n.k) {
      case "b": return <b key={i}>{kids()}</b>;
      case "i": return <i key={i}>{kids()}</i>;
      case "u": return <u key={i}>{kids()}</u>;
      case "s": return <s key={i}>{kids()}</s>;
      case "sup": return <sup key={i}>{kids()}</sup>;
      case "sub": return <sub key={i}>{kids()}</sub>;
      case "code": return <code key={i}>{kids()}</code>;
      case "pre": return <pre key={i} className="dpre">{kids()}</pre>;
      case "size": return <span key={i} style={{ fontSize: `${n.v}em` }}>{kids()}</span>;
      // the author's colours were picked for a dark page, so they show only in a dark scheme (app.css)
      case "color": return <span key={i} className="dcol" style={{ "--dc": n.v } as CSSProperties}>{kids()}</span>;
      case "div": return <div key={i} style={n.v ? { textAlign: n.v as CSSProperties["textAlign"] } : undefined}>{kids()}</div>;
      case "quote": return <blockquote key={i} className="dquote">{kids()}</blockquote>;
      case "spoiler": return <details key={i} className="dspoiler"><summary>Spoiler</summary><div>{kids()}</div></details>;
      case "h": return <div key={i} className="dh">{kids()}</div>;
      case "ul": return <ul key={i} className="dlist">{kids()}</ul>;
      case "ol": return <ol key={i} className="dlist">{kids()}</ol>;
      case "li": return <li key={i}>{kids()}</li>;
      case "table": return <div key={i} className="dtable"><table><tbody>{kids()}</tbody></table></div>;
      case "tr": return <tr key={i}>{kids()}</tr>;
      case "th": return <th key={i}>{kids()}</th>;
      case "td": return <td key={i}>{kids()}</td>;
      case "hr": return <hr key={i} className="dline" />;
      case "link": return inLink ? <span key={i}>{kids()}</span> : link(n.v!, kids(), i);
      case "img": {
        const pic = <img src={n.v} alt="" loading="lazy" referrerPolicy="no-referrer" style={n.w ? { width: `${n.w}px` } : undefined}
          onError={(e) => { e.currentTarget.style.display = "none"; }} />;
        return inLink ? <span key={i} className="dimg">{pic}</span>
          : <button key={i} type="button" className="dimg" onClick={() => setOpen(media.indexOf(n))} title="View full window">{pic}</button>;
      }
      case "video": return (
        <button key={i} type="button" className="dimg dvideo" onClick={() => setOpen(media.indexOf(n))} title="Play video">
          <img src={`https://img.youtube.com/vi/${n.v}/hqdefault.jpg`} alt="" loading="lazy" referrerPolicy="no-referrer" />
          <span className="gplay"><Icon name="play" /></span>
        </button>
      );
    }
    return <span key={i}>{kids()}</span>;
  });

  return <>
    {render(nodes)}
    {open !== null && open >= 0 && <Lightbox items={media.map(toShot)} start={open} onClose={() => setOpen(null)} />}
  </>;
}
