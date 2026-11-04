// Nexus descriptions are BBCode mixed with HTML. The app shows them as plain text: tags are dropped, line breaks
// and list items kept. Output is rendered as React text, so nothing in it can run.

const ENTITIES: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " ", "#39": "'" };

export function descriptionText(src: string): string {
  let s = src.replace(/\r\n?/g, "\n");
  s = s.replace(/<br\s*\/?>/gi, "\n").replace(/<\/(p|div|li|h\d)>/gi, "\n");
  s = s.replace(/\[\*\]/g, "\n• ").replace(/<li[^>]*>/gi, "• ");
  s = s.replace(/\[(img|youtube)\][\s\S]*?\[\/\1\]/gi, "");
  s = s.replace(/\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi, "$2");
  s = s.replace(/\[\/?[a-z*]+(=[^\]]*)?\]/gi, "");
  s = s.replace(/<[^>]+>/g, "");
  s = s.replace(/&(#?\w+);/g, (m, e: string) => ENTITIES[e] ?? (e.startsWith("#") ? String.fromCharCode(Number(e.slice(1)) || 32) : m));
  return s.split("\n").map((l) => l.replace(/[ \t]+/g, " ").trim()).join("\n").replace(/\n{3,}/g, "\n\n").trim();
}

export function shortDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const sameYear = d.getFullYear() === new Date().getFullYear();
  return d.toLocaleDateString("en-GB", sameYear ? { day: "numeric", month: "short" } : { day: "numeric", month: "short", year: "numeric" });
}

export const daysAgo = (n: number, now = Date.now()) => Math.floor(now / 1000) - n * 86400;
