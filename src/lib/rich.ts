// Nexus descriptions as a tree the app can lay out: BBCode and the HTML Nexus mixes into it, read into a small set
// of known elements (bold, sizes, alignment, lists, tables, quotes, spoilers, links, pictures, videos). Anything else
// is dropped and its text kept. The tree holds only text and those elements, never markup, so nothing in it runs.
import { linkUrl } from "./text";

export type Kind =
  | "b" | "i" | "u" | "s" | "sup" | "sub" | "code" | "span" | "size" | "color"
  | "div" | "pre" | "quote" | "spoiler" | "h" | "ul" | "ol" | "li" | "table" | "tr" | "th" | "td" | "hr"
  | "link" | "img" | "video";

/** An element: its kind, the tag it was written with (to match its close), and what it needs. */
export interface RichEl {
  k: Kind; tag: string; kids: RichNode[];
  /** link and img: the address. video: the YouTube id. size: the scale. color: a CSS colour. div: the alignment. */
  v?: string;
  /** img: the width it was given, in pixels. */
  w?: number;
}
export type RichNode = string | RichEl;

const BLOCK = new Set<Kind>(["div", "pre", "quote", "spoiler", "h", "ul", "ol", "li", "table", "tr", "th", "td", "hr"]);
const ENTITIES: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " ", "#39": "'" };
const decode = (s: string) => s.replace(/&(#x?[0-9a-f]+|\w+);/gi, (m, e: string) => {
  if (ENTITIES[e.toLowerCase()]) return ENTITIES[e.toLowerCase()];
  if (e[0] !== "#") return m;
  const n = e[1] === "x" || e[1] === "X" ? parseInt(e.slice(2), 16) : Number(e.slice(1));
  return n > 0 && n < 0x110000 ? String.fromCodePoint(n) : m;
});

// Nexus sizes run 1 to 7 and most text is written at 2 or 3; both read as normal text here
const SIZES: Record<string, string> = { 1: ".85", 2: "1", 3: "1", 4: "1.15", 5: "1.35", 6: "1.6", 7: "1.9" };
const HTML_SIZES: Record<string, string> = { "xx-small": "1", "x-small": "1", small: "2", medium: "3", large: "4", "x-large": "5", "xx-large": "6" };

/** A colour an author picked, unless it is a grey, white or black (those were picked for Nexus's own background). */
export function authorColor(raw: string): string | null {
  const c = raw.trim().replace(/^["']|["']$/g, "").toLowerCase();
  let hex = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/.exec(c)?.[1];
  if (!hex) {
    if (!/^[a-z]{3,20}$/.test(c) || ["white", "black", "gray", "grey", "silver", "lightgray", "lightgrey", "darkgray", "darkgrey", "gainsboro", "whitesmoke", "dimgray", "dimgrey", "inherit", "initial"].includes(c)) return null;
    return c;
  }
  if (hex.length === 3) hex = [...hex].map((x) => x + x).join("");
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(hex!.slice(i, i + 2), 16));
  return Math.max(r, g, b) - Math.min(r, g, b) < 40 ? null : `#${hex}`;
}

/** The YouTube id in an address or a bare id. */
export function youtubeId(s: string): string | null {
  const t = s.trim();
  if (/^[A-Za-z0-9_-]{11}$/.test(t)) return t;
  return t.match(/(?:youtube(?:-nocookie)?\.com\/(?:watch\?(?:.*&)?v=|embed\/|shorts\/)|youtu\.be\/)([A-Za-z0-9_-]{11})/)?.[1] ?? null;
}

const attr = (attrs: string, name: string) => {
  const m = new RegExp(`(?:^|\\s)${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s"'>]+))`, "i").exec(attrs);
  return m ? decode(m[1] ?? m[2] ?? m[3] ?? "") : null;
};
const styleOf = (attrs: string, prop: string) => new RegExp(`(?:^|;)\\s*${prop}\\s*:\\s*([^;]+)`, "i").exec(attr(attrs, "style") ?? "")?.[1].trim() ?? null;
const align = (v: string | null) => { const a = v?.trim().toLowerCase(); return a === "center" || a === "right" || a === "left" || a === "justify" ? a : undefined; };
const px = (v: string | null) => { const n = parseInt(v ?? "", 10); return n > 0 && n < 4000 ? n : undefined; };

// tags whose content is taken as written, up to their close
const RAW = new Set(["img", "youtube", "code", "url", "email"]);
const TOKENS = /\[(\/?)([a-z*]+)(?:=([^\]]*)|(\s[^\]]*))?\]|<(\/?)([a-z][a-z0-9]*)(\s[^<>]*?)?\s*\/?>|<!--[\s\S]*?-->/gi;

/** Read a description into the tree. */
export function parseRich(src: string): RichNode[] {
  let s = src.replace(/\r\n?/g, "\n");
  // Nexus stores each line break twice, as a newline and as <br />; HTML ignores the newline, and so does this
  const html = /<br\s*\/?>/i.test(s);
  if (html) s = s.replace(/[ \t]*\n[ \t]*(?=<br)/gi, "").replace(/(<br\s*\/?>)[ \t]*\n[ \t]*/gi, "$1").replace(/[ \t]*\n[ \t]*/g, " ");

  const root: RichEl = { k: "div", tag: "", kids: [] };
  const stack: RichEl[] = [root];
  const top = () => stack[stack.length - 1];
  const text = (t: string) => {
    if (!t) return;
    const kids = top().kids;
    const d = decode(t);
    if (typeof kids[kids.length - 1] === "string") kids[kids.length - 1] += d; else kids.push(d);
  };
  const add = (el: RichEl) => top().kids.push(el);
  const open = (el: RichEl) => { add(el); stack.push(el); };
  const close = (tag: string) => {
    for (let i = stack.length - 1; i > 0; i--) if (stack[i].tag === tag) { stack.length = i; return; }
  };
  /** Close back to the nearest of `kinds`, stopping at `fence` (an li closes inside its own list only). */
  const closeTo = (kinds: Kind[], fence: Kind[]) => {
    for (let i = stack.length - 1; i > 0; i--) {
      if (kinds.includes(stack[i].k)) { stack.length = i; return; }
      if (fence.includes(stack[i].k)) return;
    }
  };
  const media = (url: string, w?: number) => {
    const id = youtubeId(url);
    if (id && !/\.(png|jpe?g|gif|webp)(\?|$)/i.test(url)) return add({ k: "video", tag: "", kids: [], v: id });
    const u = linkUrl(decode(url));
    if (u) add({ k: "img", tag: "", kids: [], v: u, w });
  };

  let at = 0;
  const TOKEN = new RegExp(TOKENS.source, "gi"); // its own: link words are read by a call of this inside the loop
  for (let m = TOKEN.exec(s); m; m = TOKEN.exec(s)) {
    const start = m.index;
    if (m[0].startsWith("<!--")) { text(s.slice(at, start)); at = TOKEN.lastIndex; continue; }
    const bb = m[2] !== undefined;
    const closing = (bb ? m[1] : m[5]) === "/";
    const name = (bb ? m[2] : m[6]).toLowerCase();
    const eq = bb ? m[3] : undefined;
    const attrs = (bb ? m[4] : m[7]) ?? "";
    const tag = `${bb ? "[" : "<"}${name}`;

    if (bb && !closing && RAW.has(name)) {
      const end = s.toLowerCase().indexOf(`[/${name}]`, TOKEN.lastIndex);
      if (end < 0) continue; // no close: leave it as text
      text(s.slice(at, start));
      const inner = s.slice(TOKEN.lastIndex, end);
      at = end + name.length + 3;
      TOKEN.lastIndex = at;
      if (name === "img") media(inner.trim(), px(attr(attrs, "width")));
      else if (name === "youtube") media(inner.trim());
      else if (name === "code") add({ k: inner.includes("\n") || inner.includes("<br") ? "pre" : "code", tag, kids: [decode(inner.replace(/<br\s*\/?>/gi, "\n"))] });
      else if (name === "email") text(inner);
      else {
        // [url]address[/url], or [url=address]words[/url] (whose words can hold more tags, so parse them)
        const u = linkUrl(decode(eq ?? inner));
        if (eq === undefined) { if (u) add({ k: "link", tag, kids: [inner.trim()], v: u }); else text(inner); }
        else {
          const inside = parseRich(inner);
          if (u) add({ k: "link", tag, kids: inside, v: u }); else top().kids.push(...inside);
        }
      }
      continue;
    }

    const el = closing ? null : element(bb, name, eq, attrs, tag);
    const known = closing ? stack.some((x) => x.tag === tag) || KNOWN.has(tag) : el !== undefined;
    if (!known) {
      // an unknown BBCode tag is text someone wrote ("[REQUIRED]"); an unknown HTML tag is dropped
      if (!bb) { text(s.slice(at, start)); at = TOKEN.lastIndex; }
      continue;
    }
    text(s.slice(at, start));
    at = TOKEN.lastIndex;
    if (closing) { close(tag); continue; }
    if (!el) continue; // a close, or an <img> with no picture
    if (el.k === "li") closeTo(["li"], ["ul", "ol"]);
    if (el.k === "td" || el.k === "th") closeTo(["td", "th"], ["tr", "table"]);
    if (el.k === "tr") closeTo(["tr"], ["table"]);
    if (el.k === "hr" || el.k === "img" || el.k === "video") { add(el); continue; }
    if (el.k === "span" && el.tag === "<br") { text("\n"); continue; }
    open(el);
  }
  text(s.slice(at));
  return tidy(root.kids, "div");
}

const KNOWN = new Set([
  "[b", "[i", "[u", "[s", "[strike", "[sup", "[sub", "[size", "[color", "[font", "[center", "[left", "[right", "[justify",
  "[quote", "[spoiler", "[heading", "[line", "[hr", "[list", "[*", "[table", "[tr", "[th", "[td", "[indent",
  "<b", "<strong", "<i", "<em", "<u", "<s", "<strike", "<del", "<sup", "<sub", "<span", "<font", "<p", "<div", "<center",
  "<h1", "<h2", "<h3", "<h4", "<h5", "<h6", "<ul", "<ol", "<li", "<blockquote", "<pre", "<code", "<table", "<thead",
  "<tbody", "<tfoot", "<tr", "<th", "<td", "<a", "<img", "<hr", "<br", "<iframe",
]);

/** The element a tag opens: undefined for a tag this does not know, null for one it knows that adds nothing. */
function element(bb: boolean, name: string, eq: string | undefined, attrs: string, tag: string): RichEl | null | undefined {
  if (!KNOWN.has(tag)) return undefined;
  const el = (k: Kind, v?: string, w?: number): RichEl => ({ k, tag, kids: [], v, w });
  if (bb) {
    switch (name) {
      case "b": case "i": case "u": case "s": case "sup": case "sub": return el(name);
      case "strike": return el("s");
      case "size": { const n = SIZES[(eq ?? "").trim()]; return n && n !== "1" ? el("size", n) : el("span"); }
      case "color": { const c = authorColor(eq ?? ""); return c ? el("color", c) : el("span"); }
      case "font": case "indent": return el("span");
      case "center": case "left": case "right": case "justify": return el("div", name);
      case "quote": return el("quote");
      case "spoiler": return el("spoiler");
      case "heading": return el("h");
      case "line": case "hr": return el("hr");
      case "list": return el(eq && /^[1ai]/i.test(eq.trim()) ? "ol" : "ul");
      case "*": return el("li");
      case "table": case "tr": case "th": case "td": return el(name);
    }
    return undefined;
  }
  switch (name) {
    case "b": case "strong": return el("b");
    case "i": case "em": return el("i");
    case "u": case "sup": case "sub": case "code": return el(name as Kind);
    case "s": case "strike": case "del": return el("s");
    case "span": case "font": {
      const c = authorColor(attr(attrs, "color") ?? styleOf(attrs, "color") ?? "");
      const size = HTML_SIZES[styleOf(attrs, "font-size") ?? ""] ?? (name === "font" ? attr(attrs, "size") : null);
      const n = size ? SIZES[size] : undefined;
      return c ? el("color", c) : n && n !== "1" ? el("size", n) : el("span");
    }
    case "p": case "div": case "center": return el("div", name === "center" ? "center" : align(attr(attrs, "align") ?? styleOf(attrs, "text-align")));
    case "h1": case "h2": case "h3": case "h4": case "h5": case "h6": return el("h");
    case "ul": case "ol": case "li": case "pre": case "table": case "tr": case "th": case "td": return el(name);
    case "blockquote": return el("quote");
    case "thead": case "tbody": case "tfoot": return el("span");
    case "hr": return el("hr");
    case "br": return el("span");
    case "a": { const u = linkUrl(attr(attrs, "href") ?? ""); return u ? el("link", u) : el("span"); }
    case "img": { const u = attr(attrs, "src"); return u ? mediaEl(u, px(attr(attrs, "width") ?? styleOf(attrs, "width"))) : null; }
    case "iframe": { const id = youtubeId(attr(attrs, "src") ?? ""); return id ? { k: "video", tag: "", kids: [], v: id } : null; }
  }
  return undefined;
}

function mediaEl(url: string, w?: number): RichEl | null {
  const u = linkUrl(url);
  return u ? { k: "img", tag: "", kids: [], v: u, w } : null;
}

/** Tidy the tree: the line break written next to a block is the block's own and goes, whitespace between table
 *  cells goes, a span that adds nothing is flattened, and the description's ends are trimmed. */
function tidy(kids: RichNode[], parent: Kind): RichNode[] {
  const out: RichNode[] = [];
  for (const n of kids) {
    if (typeof n === "string") { out.push(n); continue; }
    const k = { ...n, kids: tidy(n.kids, n.k) };
    if (k.k === "span") out.push(...k.kids); else out.push(k);
  }
  // join text that flattening put side by side
  const joined: RichNode[] = [];
  for (const n of out) {
    if (typeof n === "string" && typeof joined[joined.length - 1] === "string") joined[joined.length - 1] += n;
    else joined.push(n);
  }
  const res: RichNode[] = [];
  for (let i = 0; i < joined.length; i++) {
    let n = joined[i];
    if (typeof n !== "string") { res.push(n); continue; }
    const prev = joined[i - 1], next = joined[i + 1];
    if (prev && typeof prev !== "string" && BLOCK.has(prev.k)) n = n.replace(/^[ \t]*\n/, "");
    if (next && typeof next !== "string" && BLOCK.has(next.k)) n = n.replace(/\n[ \t]*$/, "");
    if ((parent === "table" || parent === "tr" || parent === "ul" || parent === "ol") && !n.trim()) continue;
    if (n) res.push(n);
  }
  return BLOCK.has(parent) ? trimEnds(res) : res;
}

function trimEnds(kids: RichNode[]): RichNode[] {
  const k = [...kids];
  while (typeof k[0] === "string" && !k[0].trim()) k.shift();
  while (typeof k[k.length - 1] === "string" && !(k[k.length - 1] as string).trim()) k.pop();
  if (typeof k[0] === "string") k[0] = k[0].replace(/^\s+/, "");
  if (typeof k[k.length - 1] === "string") k[k.length - 1] = (k[k.length - 1] as string).replace(/\s+$/, "");
  return k;
}

/** The pictures and videos in a tree, in the order they appear. */
export function richMedia(nodes: RichNode[]): RichEl[] {
  const out: RichEl[] = [];
  const walk = (ns: RichNode[]) => { for (const n of ns) if (typeof n !== "string") { if (n.k === "img" || n.k === "video") out.push(n); else walk(n.kids); } };
  walk(nodes);
  return out;
}
