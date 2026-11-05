// Nexus descriptions are BBCode mixed with HTML. The app shows them as plain text with their links: tags are
// dropped, line breaks, list items and links kept. Output is rendered as React text and links, so nothing in it runs.

const ENTITIES: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " ", "#39": "'" };

/** A run of description text, or a link in it (its words as written, and where it goes). */
export type DescPart = string | { text: string; url: string };

// links are held as \u0001url\u0002words\u0003 while the tags around them are dropped
const LINK = /\u0001([^\u0002]*)\u0002([^\u0003]*)\u0003/g;
const BARE = /\bhttps?:\/\/[^\s<>"'\u0001-\u0003]+/gi;

/** A link's address, made whole: quotes trimmed, www. and Nexus's own paths given their site. Null when it is not a web page. */
export function linkUrl(raw: string): string | null {
  let u = raw.trim().replace(/^["']|["']$/g, "").trim();
  if (u.startsWith("//")) u = `https:${u}`;
  else if (u.startsWith("/")) u = `https://www.nexusmods.com${u}`;
  else if (/^www\./i.test(u)) u = `https://${u}`;
  if (!/^https?:\/\/[^/\s]+\.[^/\s]+/i.test(u)) return null;
  try { return new URL(u).href; } catch { return null; }
}

export function descriptionParts(src: string): DescPart[] {
  let s = src.replace(/\r\n?/g, "\n").replace(/[\u0001-\u0003]/g, "");
  const hold = (url: string, words: string) => { const u = linkUrl(url.replace(/&amp;/g, "&")); return u ? `\u0001${u}\u0002${words}\u0003` : words; };
  s = s.replace(/\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi, (_, u: string, t: string) => hold(u, t));
  s = s.replace(/\[url\]\s*([^[\s]+)\s*\[\/url\]/gi, (_, u: string) => hold(u, ""));
  s = s.replace(/<a\s[^>]*?href\s*=\s*(["'])([^"']+)\1[^>]*>([\s\S]*?)<\/a>/gi, (_, _q: string, u: string, t: string) => hold(u, t));
  s = s.replace(/<br\s*\/?>/gi, "\n").replace(/<\/(p|div|li|h\d)>/gi, "\n");
  s = s.replace(/\[\*\]/g, "\n• ").replace(/<li[^>]*>/gi, "• ");
  s = s.replace(/\[(img|youtube)\][\s\S]*?\[\/\1\]/gi, "");
  s = s.replace(/\[\/?[a-z*]+(=[^\]]*)?\]/gi, "");
  s = s.replace(/<[^>]+>/g, "");
  s = s.replace(/&(#?\w+);/g, (m, e: string) => ENTITIES[e] ?? (e.startsWith("#") ? String.fromCharCode(Number(e.slice(1)) || 32) : m));
  s = s.split("\n").map((l) => l.replace(/[ \t]+/g, " ").trim()).join("\n").replace(/\n{3,}/g, "\n\n").trim();
  const out: DescPart[] = [];
  const text = (t: string) => {
    // bare addresses in the text are links too; a full stop or bracket after one is not part of it
    let at = 0;
    for (const m of t.matchAll(BARE)) {
      const raw = m[0].replace(/[.,;:!?)\]}'"]+$/, "");
      const u = linkUrl(raw);
      if (!u) continue;
      const i = m.index ?? 0;
      if (i > at) out.push(t.slice(at, i));
      out.push({ text: raw, url: u });
      at = i + raw.length;
    }
    if (at < t.length) out.push(t.slice(at));
  };
  let at = 0;
  for (const m of s.matchAll(LINK)) {
    const i = m.index ?? 0;
    if (i > at) text(s.slice(at, i));
    const words = m[2].replace(/[ \t]+/g, " ").trim();
    out.push({ text: words || m[1], url: m[1] });
    at = i + m[0].length;
  }
  if (at < s.length) text(s.slice(at));
  return out;
}

export function descriptionText(src: string): string {
  return descriptionParts(src).map((p) => (typeof p === "string" ? p : p.text)).join("");
}

/** The same address written two ways (http, a trailing slash, nexusmods.com without www) is one link. */
export const linkKey = (url: string) => url.replace(/#.*$/, "").replace(/\/$/, "").replace(/^http:/, "https:").replace("://nexusmods.com", "://www.nexusmods.com");

/** Every link in a description once, in the order they appear, with the words each was written with. */
export function descriptionLinks(parts: DescPart[]): Array<{ url: string; labels: string[] }> {
  const by = new Map<string, { url: string; labels: string[] }>();
  for (const p of parts) {
    if (typeof p === "string") continue;
    const key = linkKey(p.url);
    let have = by.get(key);
    if (!have) { have = { url: p.url, labels: [] }; by.set(key, have); }
    if (!/^(https?:\/\/|www\.)/i.test(p.text) && !have.labels.includes(p.text)) have.labels.push(p.text);
  }
  return [...by.values()];
}

export function shortDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const sameYear = d.getFullYear() === new Date().getFullYear();
  return d.toLocaleDateString("en-GB", sameYear ? { day: "numeric", month: "short" } : { day: "numeric", month: "short", year: "numeric" });
}

export const daysAgo = (n: number, now = Date.now()) => Math.floor(now / 1000) - n * 86400;

/** "1 mod", "3 mods": a count with its word in the right number. */
export function plural(n: number, one: string, many = `${one}s`): string { return `${n} ${n === 1 ? one : many}`; }
