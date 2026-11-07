// Links in mod and collection descriptions: clickable where they are written (DescText), and every one of them
// in a viewer at the top of the page (Links), previewed like a chat app shows a link: picture, site, title and
// summary. Sorted by what they are (mods, videos, guides, downloads, community, support) and searchable. Previews
// load only once the viewer is opened, so a page never reaches other sites just by being looked at.
import { useEffect, useMemo, useState, type MouseEvent, type ReactNode } from "react";
import { Icon } from "../../ui/Icon";
import type { IconName } from "../../ui/icons";
import { Key, Seg, formatCount } from "../../ui/pocket";
import { errorText, links as linksApi, nexusPages, type LinkPreview } from "../../lib/api";
import { linkKey, type DescPart } from "../../lib/text";
import { toast } from "../../ui/toast";

/** A Cyberpunk 2077 mod page on Nexus: its id. */
export function nexusModId(url: string): number | null {
  const m = url.match(/^https?:\/\/(?:www\.|next\.)?nexusmods\.com\/cyberpunk2077\/mods\/(\d+)/i);
  return m ? Number(m[1]) : null;
}
const isNexus = (url: string) => /^https?:\/\/([a-z]+\.)?nexusmods\.com\//i.test(url);

/** Open a description's link: a Nexus mod here (onMod) or where pages open, another Nexus page where pages open,
 *  anything else in the user's own browser. `own` (Ctrl, Shift or middle click) always uses the user's browser. */
export function openLink(url: string, onMod?: (modId: number) => void, own = false) {
  const fail = (e: unknown) => toast(errorText(e), true);
  const id = nexusModId(url);
  if (!own && id && onMod) return onMod(id);
  if (!own && id) return void nexusPages.mod(id).catch(fail);
  if (!own && isNexus(url)) return void nexusPages.url(url.replace(/^https?:\/\/(nexusmods\.com)/i, "https://www.$1").replace(/^http:/, "https:")).catch(fail);
  void linksApi.open(url).catch(fail);
}

export const tipOf = (url: string) =>
  nexusModId(url) || isNexus(url) ? `${url}\nCtrl+click opens in your browser` : url;

/** A description's text with its links clickable. `limit` cuts it after that many characters. */
export function DescText({ parts, limit, onMod }: { parts: DescPart[]; limit?: number; onMod?: (modId: number) => void }) {
  const out: ReactNode[] = [];
  let left = limit ?? Infinity;
  for (const [i, p] of parts.entries()) {
    if (left <= 0) break;
    if (typeof p === "string") { out.push(p.length > left ? p.slice(0, left) : p); left -= p.length; continue; }
    const go = (e: MouseEvent) => { e.preventDefault(); openLink(p.url, onMod, e.ctrlKey || e.metaKey || e.shiftKey || e.button === 1); };
    out.push(<a key={i} href={p.url} className="dlink" onClick={go} onAuxClick={(e) => { if (e.button === 1) go(e); }} title={tipOf(p.url)}>{p.text.length > left ? p.text.slice(0, left) : p.text}</a>);
    left -= p.text.length;
  }
  return <>{out}</>;
}

// ── previews: fetched once each, four at a time, kept for the session (the app keeps them a week) ──

const got = new Map<string, LinkPreview>();
const waiting = new Map<string, Promise<LinkPreview>>();
const queue: Array<() => void> = [];
let running = 0;
function preview(url: string): Promise<LinkPreview> {
  let p = waiting.get(url);
  if (p) return p;
  p = new Promise<LinkPreview>((done) => {
    const go = () => {
      running++;
      linksApi.preview(url)
        .catch((e): LinkPreview => ({ url, finalUrl: url, kind: "page", error: errorText(e) }))
        .then((v) => { got.set(url, v); done(v); })
        .finally(() => { running--; waiting.delete(url); queue.shift()?.(); });
    };
    if (running < 4) go(); else queue.push(go);
  });
  waiting.set(url, p);
  return p;
}
function usePreviews(urls: string[], on: boolean) {
  const [, bump] = useState(0);
  const key = urls.join("\n");
  useEffect(() => {
    if (!on) return;
    let live = true;
    for (const u of urls) if (!got.has(u)) void preview(u).then(() => { if (live) bump((n) => n + 1); });
    return () => { live = false; };
  }, [on, key]); // eslint-disable-line react-hooks/exhaustive-deps
  return got;
}

// ── what a link is ──

type Cat = "mods" | "videos" | "guides" | "files" | "community" | "support" | "other";
const CATS: Array<{ key: Cat; label: string; short: string; icon: IconName }> = [
  { key: "mods", label: "On Nexus Mods", short: "Nexus", icon: "library" },
  { key: "videos", label: "Videos", short: "Videos", icon: "play" },
  { key: "guides", label: "Guides and wikis", short: "Guides", icon: "info" },
  { key: "files", label: "Downloads and code", short: "Downloads", icon: "downloads" },
  { key: "community", label: "Community", short: "Community", icon: "user" },
  { key: "support", label: "Support the author", short: "Support", icon: "star" },
  { key: "other", label: "Other sites", short: "Other", icon: "web" },
];
const hostOf = (url: string) => { try { return new URL(url).hostname.replace(/^www\./, "").toLowerCase(); } catch { return url; } };
const on = (h: string, list: string[]) => list.some((d) => h === d || h.endsWith(`.${d}`));
function catOf(url: string): Cat {
  const h = hostOf(url);
  if (isNexus(url)) return "mods";
  if (on(h, ["youtube.com", "youtu.be", "twitch.tv", "vimeo.com", "streamable.com", "bilibili.com"])) return "videos";
  if (on(h, ["ko-fi.com", "patreon.com", "paypal.com", "paypal.me", "buymeacoffee.com", "boosty.to", "liberapay.com", "subscribestar.com"]) || /github\.com\/sponsors\//i.test(url)) return "support";
  if (on(h, ["discord.gg", "discord.com", "discordapp.com", "reddit.com", "redd.it", "twitter.com", "x.com", "bsky.app", "steamcommunity.com", "instagram.com", "tiktok.com", "facebook.com", "threads.net", "mastodon.social", "forums.cdprojektred.com"])) return "community";
  if (/wiki|docs?\.|gitbook|readthedocs|fandom\.com/i.test(h) || /\/(wiki|docs?|guide)s?\b/i.test(new URL(url).pathname)) return "guides";
  if (on(h, ["github.com", "gitlab.com", "codeberg.org", "drive.google.com", "mega.nz", "mediafire.com", "dropbox.com", "1drv.ms", "onedrive.live.com", "modding.wiki", "gamebanana.com", "mod.io", "pastebin.com", "gist.github.com"])) return "files";
  return "other";
}

/** Every link in the description, previewed, at the top of the page in a dropdown that opens (like Console commands). */
export function Links({ list, onMod }: { list: Array<{ url: string; labels: string[] }>; onMod?: (modId: number) => void }) {
  // opened once, it stays open on the next page
  const [open, setOpenState] = useState(() => { try { return localStorage.getItem("pm.links.open") === "1"; } catch { return false; } });
  const setOpen = (v: boolean) => { setOpenState(v); try { localStorage.setItem("pm.links.open", v ? "1" : "0"); } catch { /* storage unavailable */ } };
  const [q, setQ] = useState("");
  const [cat, setCat] = useState<Cat | "all">("all");
  const previews = usePreviews(list.map((l) => l.url), open);
  const withCat = useMemo(() => list.map((l) => ({ ...l, cat: catOf(l.url) })), [list]);
  const present = CATS.filter((c) => withCat.some((l) => l.cat === c.key));
  const t = q.trim().toLowerCase();
  const shown = withCat.filter((l) => {
    if (cat !== "all" && l.cat !== cat) return false;
    if (!t) return true;
    const p = previews.get(l.url);
    return `${l.url} ${l.labels.join(" ")} ${p?.title ?? ""} ${p?.description ?? ""} ${p?.site ?? ""} ${p?.author ?? ""}`.toLowerCase().includes(t);
  });
  const hosts = [...new Set(list.map((l) => hostOf(l.url)))];
  const copy = (text: string) => navigator.clipboard.writeText(text).then(() => toast("Link copied")).catch(() => toast("Couldn't copy", true));
  return (
    <div className="cmds links">
      <button type="button" className="cmds-h" aria-expanded={open} onClick={() => setOpen(!open)} title={open ? "Collapse" : "Links in the description"}>
        <Icon name="chevron" size={9} className={open ? "rot" : ""} />
        <b>Links</b><span className="n">{list.length}</span>
        <span className="sp" />
        <span className="dim lnk-hosts">{hosts.slice(0, 3).join(", ")}{hosts.length > 3 ? ` and ${hosts.length - 3} more` : ""}</span>
      </button>
      {open && <>
        {list.length > 4 && (
          <div className="lnk-tools">
            <label className="fbox"><Icon name="browse" />
              <input className="finput" type="text" value={q} onChange={(e) => setQ(e.target.value)} placeholder="Find a link" aria-label="Find a link"
                onKeyDown={(e) => { if (e.key === "Escape" && q) { e.stopPropagation(); setQ(""); } }} />
            </label>
            {present.length > 1 && <Seg label="Kind" value={cat} onChange={setCat}
              options={[{ value: "all", label: "All" }, ...present.map((c) => ({ value: c.key, label: `${c.short} ${withCat.filter((l) => l.cat === c.key).length}`, title: c.label }))]} />}
          </div>
        )}
        <div className="lnk-list">
          {shown.length === 0 && <p className="dim" style={{ fontSize: ".86em", padding: ".3em" }}>No matches <button type="button" className="linkish" onClick={() => { setQ(""); setCat("all"); }}>Show all</button></p>}
          {CATS.map((c) => {
            const here = shown.filter((l) => l.cat === c.key);
            if (!here.length) return null;
            return (
              <div key={c.key} className="lnk-sec">
                {present.length > 1 && <div className="lnk-h"><Icon name={c.icon} size={9} /> {c.label} <span className="dim">{here.length}</span></div>}
                <div className="lnk-grid">
                  {here.map((l) => <LinkCard key={linkKey(l.url)} url={l.url} labels={l.labels} p={previews.get(l.url)} onMod={onMod} onCopy={copy} />)}
                </div>
              </div>
            );
          })}
        </div>
        {list.length > 1 && <Key size="sm" icon="file" onClick={() => copy(list.map((l) => l.url).join("\n"))} title="One per line">Copy all</Key>}
      </>}
    </div>
  );
}

function LinkCard({ url, labels, p, onMod, onCopy }: { url: string; labels: string[]; p?: LinkPreview; onMod?: (modId: number) => void; onCopy: (t: string) => void }) {
  const [imgOk, setImgOk] = useState(true);
  const [iconOk, setIconOk] = useState(true);
  const host = hostOf(url);
  const u = (() => { try { return new URL(url); } catch { return null; } })();
  const path = u ? decodeURIComponent(`${u.pathname}${u.search}`).replace(/\/$/, "") : "";
  const modId = nexusModId(url);
  const title = p?.title || labels[0] || (path ? `${host}${path}` : host);
  const video = p?.kind === "video" || catOf(url) === "videos";
  const fallback: IconName = modId ? "library" : video ? "play" : p?.kind === "file" ? "file" : p?.kind === "invite" ? "user" : "web";
  const go = (e: MouseEvent) => openLink(url, onMod, e.ctrlKey || e.metaKey || e.shiftKey || e.button === 1);
  return (
    <div className={`lnk${p ? "" : " loading"}${p?.error ? " err" : ""}`}>
      <button type="button" className="lnk-main" onClick={go} onAuxClick={(e) => { if (e.button === 1) go(e); }} title={tipOf(url)}>
        <span className={`lnk-img${video ? " video" : ""}`}>
          {p?.image && imgOk ? <img src={p.image} alt="" loading="lazy" referrerPolicy="no-referrer" onError={() => setImgOk(false)} /> : <Icon name={fallback} size={27} />}
          {video && p?.image && imgOk && <span className="lnk-play"><Icon name="play" size={18} /></span>}
        </span>
        <span className="lnk-b">
          <span className="lnk-site">
            {p?.icon && iconOk && <img src={p.icon} alt="" referrerPolicy="no-referrer" onError={() => setIconOk(false)} />}
            <span>{p?.site && p.site.toLowerCase() !== host ? `${p.site} · ${host}` : host}</span>
          </span>
          <span className="lnk-t">{title}</span>
          {p?.description && <span className="lnk-d">{p.description}</span>}
          {!p && <span className="lnk-d dim">Loading</span>}
          {p?.error && <span className="lnk-d dim">{p.error}</span>}
          <span className="lnk-m">
            {modId && p?.author && <span>by {p.author}</span>}
            {p?.downloads != null && <span>{formatCount(p.downloads)} downloads</span>}
            {p?.endorsements != null && <span>{formatCount(p.endorsements)} endorsements</span>}
            {labels.length > 0 && labels.some((l) => l !== title) && <span className="lnk-w" title="Link text">“{labels.filter((l) => l !== title).slice(0, 2).join("”, “")}”</span>}
            {!modId && path && path.length > 1 && <span className="mono lnk-p">{path}</span>}
          </span>
        </span>
      </button>
      <span className="lnk-k">
        <Key size="sm" variant="ghost" icon="file" aria-label={`Copy ${url}`} title="Copy link" onClick={() => onCopy(url)} />
        {(modId || isNexus(url)) && <Key size="sm" variant="ghost" icon="link" aria-label="Open in your browser" title="Open in your browser (Ctrl+click)" onClick={() => openLink(url, onMod, true)} />}
      </span>
    </div>
  );
}
