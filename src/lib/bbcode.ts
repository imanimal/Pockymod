// Nexus descriptions are BBCode. This turns a safe subset into a small tree the UI renders with React
// (never as HTML), so nothing in a description can run script or load content.
// Kept: b, i, u, s, size (ignored), color (ignored), center, quote, code, list/*, url (shown as text with its
// address), img (shown as a placeholder), spoiler, heading, line. Everything else keeps its inner text.

export type Node =
  | { t: "text"; v: string }
  | { t: "br" }
  | { t: "hr" }
  | { t: "b" | "i" | "u" | "s" | "center" | "quote" | "code" | "spoiler" | "h" | "p"; c: Node[] }
  | { t: "list"; ordered: boolean; items: Node[][] }
  | { t: "link"; href: string; c: Node[] }
  | { t: "img"; src: string };

const STYLE = new Set(["b", "i", "u", "s", "center", "quote", "code", "spoiler"]);
const PASS = new Set(["size", "color", "font", "left", "right", "justify", "youtube", "line", "heading", "list", "*", "url", "img", "ol", "ul", "li", "h1", "h2", "h3", "h4"]);

interface Tok {
  kind: "open" | "close" | "text";
  tag?: string;
  arg?: string;
  v?: string;
}

function tokenize(src: string): Tok[] {
  const out: Tok[] = [];
  const re = /\[(\/?)([a-z*][a-z0-9]*)(?:=([^\]]*))?\]/gi;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src))) {
    const tag = m[2].toLowerCase();
    if (!STYLE.has(tag) && !PASS.has(tag)) continue;
    if (m.index > last) out.push({ kind: "text", v: src.slice(last, m.index) });
    out.push({ kind: m[1] ? "close" : "open", tag, arg: m[3]?.replace(/^["']|["']$/g, "") });
    last = re.lastIndex;
  }
  if (last < src.length) out.push({ kind: "text", v: src.slice(last) });
  return out;
}

function decodeEntities(s: string): string {
  return s
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .replace(/&nbsp;/g, " ")
    .replace(/&quot;/g, '"')
    .replace(/&#0?39;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&");
}

function textNodes(s: string): Node[] {
  const out: Node[] = [];
  s.split("\n").forEach((line, i) => {
    if (i > 0) out.push({ t: "br" });
    if (line) out.push({ t: "text", v: line });
  });
  return out;
}

/** Only http(s) links survive; anything else (javascript:, data:) becomes plain text. */
export function safeHref(href: string | undefined): string | null {
  if (!href) return null;
  const h = href.trim();
  return /^https?:\/\//i.test(h) ? h : null;
}

export function parse(src: string): Node[] {
  const toks = tokenize(decodeEntities(src ?? ""));
  let i = 0;

  function until(end: string | null): Node[] {
    const out: Node[] = [];
    while (i < toks.length) {
      const tk = toks[i];
      if (tk.kind === "close") {
        if (end && tk.tag === end) {
          i++;
          return out;
        }
        // a stray closing tag closes the list item / list we're in, or is ignored
        if (end === "*" && (tk.tag === "list" || tk.tag === "ul" || tk.tag === "ol")) return out;
        i++;
        continue;
      }
      if (tk.kind === "text") {
        out.push(...textNodes(tk.v!));
        i++;
        continue;
      }
      const tag = tk.tag!;
      if (end === "*" && tag === "*") return out; // next item
      i++;
      if (STYLE.has(tag)) out.push({ t: tag as "b", c: until(tag) });
      else if (tag === "heading" || /^h[1-4]$/.test(tag)) out.push({ t: "h", c: until(tag) });
      else if (tag === "line") out.push({ t: "hr" });
      else if (tag === "list" || tag === "ul" || tag === "ol") {
        const ordered = tag === "ol" || (!!tk.arg && tk.arg !== "");
        const items: Node[][] = [];
        // anything before the first [*] is dropped (usually whitespace)
        while (i < toks.length) {
          const t2 = toks[i];
          if (t2.kind === "close" && t2.tag === tag) {
            i++;
            break;
          }
          if (t2.kind === "open" && (t2.tag === "*" || t2.tag === "li")) {
            i++;
            items.push(until(t2.tag === "li" ? "li" : "*"));
            continue;
          }
          i++;
        }
        out.push({ t: "list", ordered, items: items.filter((it) => it.length > 0) });
      } else if (tag === "url") {
        const inner = until("url");
        const innerText = inner.map((n) => (n.t === "text" ? n.v : "")).join("");
        const href = safeHref(tk.arg || innerText);
        out.push(href ? { t: "link", href, c: inner.length ? inner : [{ t: "text", v: href }] } : { t: "p", c: inner });
      } else if (tag === "img") {
        const inner = until("img");
        const src2 = inner.map((n) => (n.t === "text" ? n.v : "")).join("").trim();
        const href = safeHref(src2);
        if (href) out.push({ t: "img", src: href });
      } else {
        // size, color, font, alignment, youtube: keep the inside
        out.push({ t: "p", c: until(tag) });
      }
    }
    return out;
  }
  return until(null);
}

/** Plain text, for summaries and tests. */
export function toText(nodes: Node[]): string {
  return nodes
    .map((n) => {
      switch (n.t) {
        case "text":
          return n.v;
        case "br":
          return "\n";
        case "hr":
          return "\n";
        case "list":
          return n.items.map((it) => `• ${toText(it)}`).join("\n");
        case "img":
          return "";
        default:
          return toText(n.c);
      }
    })
    .join("");
}
