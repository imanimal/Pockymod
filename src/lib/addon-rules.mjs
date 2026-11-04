// The rules for add-on packages (extensions and themes) and the marketplace index. Plain JavaScript with no
// imports, so the app (src/lib/extensions.ts, src/lib/market.ts) and the marketplace check
// (scripts/validate-marketplace.mjs, run by CI with plain node) use the same rules. Types are in addon-rules.d.mts.
// The format is described for authors in docs/extensions.md and docs/marketplace.md.

export const EXTENSION_FORMAT = "pocket-mods.extension";
export const THEME_FORMAT = "pocket-mods.theme";
export const MARKET_FORMAT = "pocket-mods.marketplace";

/** What a panel may ask the app for. Each one is declared in the manifest and shown to the user before install. */
export const PERMISSIONS = {
  "library.read": { title: "See your mod list", detail: "Names, versions, groups, sizes, and which mods are on." },
  "library.toggle": { title: "Turn your mods on and off", detail: "In the active profile. A note shows each time it does." },
  "nexus.search": { title: "Search Nexus Mods", detail: "Through the app, at most 30 searches a minute." },
  "queue.add": { title: "Offer mods to download", detail: "You are asked every time. Nothing is queued unless you say yes." },
  "toast": { title: "Show short notes", detail: "In the corner of the window, with the extension's name." },
  "nexus.open": { title: "Open Nexus Mods pages", detail: "In the app's Nexus window." },
  "storage": { title: "Keep its own notes", detail: "Up to 256 KB, only for this extension, removed with it." },
};

export const CONTRIBUTIONS = ["shelves", "groupRules", "tools", "launchOptions", "commandPacks", "links", "panels"];
export const SORT_KEYS = ["downloads", "endorsements", "updated", "created", "relevance", "size", "name", "random"];
export const MOD_TYPES = ["redmod", "archive", "script", "framework"];
export const ALL_MOD_TYPES = ["redmod", "archive", "script", "framework", "mixed", "unknown"];
export const FRAMEWORK_KEYS = ["cet", "red4ext", "redscript", "archivexl", "tweakxl", "codeware", "cybercmd"];
const SHELF_FIELDS = ["text", "categories", "tags", "excludeTags", "sort", "ascending", "frameworks", "modType", "endorsementsMin",
  "downloadsMax", "createdWithinDays"];
const MATCH_FIELDS = ["name", "category", "author", "modType", "frameworks", "mods"];

export const LIMITS = { panelHtml: 512 * 1024, manifest: 1024 * 1024, storage: 256 * 1024, perList: 50, commands: 200, args: 24 };

const ID = /^[a-z0-9](?:[a-z0-9.-]{0,62}[a-z0-9])?$/;
/** Ids: lowercase letters, digits, dots and dashes, as file names on every system. */
export const isId = (s) => typeof s === "string" && ID.test(s) && !s.includes("..");
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;
export const isSemver = (s) => typeof s === "string" && SEMVER.test(s);

/** Compare two versions like 1.2.0 and 1.10.0. Anything that is not semver sorts below what is. */
export function compareVersions(a, b) {
  const pa = SEMVER.exec(String(a)), pb = SEMVER.exec(String(b));
  if (!pa || !pb) return pa ? 1 : pb ? -1 : 0;
  for (let i = 1; i <= 3; i++) { const d = Number(pa[i]) - Number(pb[i]); if (d) return d > 0 ? 1 : -1; }
  if (pa[4] === pb[4]) return 0;
  if (!pa[4]) return 1;
  if (!pb[4]) return -1;
  return pa[4] < pb[4] ? -1 : 1;
}

const isObj = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
const isText = (v, max) => typeof v === "string" && v.trim().length > 0 && v.length <= max;
const isHttps = (v) => typeof v === "string" && /^https:\/\/[^\s"<>]+$/.test(v) && v.length <= 2000;
const oneLine = (v, max) => isText(v, max) && !/[\r\n\u0000]/.test(v);
const isArg = (v) => typeof v === "string" && v.length > 0 && v.length <= 260 && !/["\u0000-\u001f\u007f]/.test(v);
const ICON = (v) => (typeof v === "string" && /^[a-z0-9]{1,20}$/.test(v))
  || (Array.isArray(v) && v.length === 9 && v.every((r) => typeof r === "string" && /^[.#]{9}$/.test(r)));

function checker() {
  const errors = [];
  const warnings = [];
  const err = (at, msg) => errors.push(at ? `${at}: ${msg}` : msg);
  const warn = (at, msg) => warnings.push(at ? `${at}: ${msg}` : msg);
  const known = (obj, at, fields) => { for (const k of Object.keys(obj)) if (!fields.includes(k)) err(at ? `${at}.${k}` : k, "is not a field the app knows. Check the spelling."); };
  const strings = (v, at, max = 60, maxLen = 30) => {
    if (!Array.isArray(v)) return err(at, "must be a list of text.");
    if (v.length > maxLen) err(at, `may have at most ${maxLen} entries.`);
    v.forEach((x, i) => { if (!oneLine(x, max)) err(`${at}[${i}]`, `must be text of up to ${max} characters.`); });
  };
  const oneOf = (v, at, list) => { if (!list.includes(v)) err(at, `must be one of ${list.join(", ")}.`); };
  const count = (v, at, max = 1e12) => { if (!Number.isInteger(v) || v < 0 || v > max) err(at, "must be a whole number, 0 or more."); };
  return { errors, warnings, err, warn, known, strings, oneOf, count };
}

function list(c, m, key, check) {
  const v = m[key];
  if (v === undefined) return;
  const at = `contributes.${key}`;
  if (!Array.isArray(v)) return c.err(at, "must be a list.");
  if (v.length > LIMITS.perList) c.err(at, `may have at most ${LIMITS.perList} entries.`);
  const seen = new Set();
  v.forEach((x, i) => {
    const here = `${at}[${i}]`;
    if (!isObj(x)) return c.err(here, "must be an object.");
    if (key !== "groupRules") {
      if (!isId(x.id)) c.err(`${here}.id`, "must be an id: lowercase letters, digits, dots and dashes.");
      else if (seen.has(x.id)) c.err(`${here}.id`, `${x.id} is used twice.`);
      else seen.add(x.id);
    }
    check(x, here);
  });
}

/** Check an extension manifest (already parsed). Errors stop the install; warnings are shown. */
export function validateExtension(m) {
  const c = checker();
  if (!isObj(m)) { c.err("", "An extension is a JSON object."); return { ok: false, errors: c.errors, warnings: c.warnings }; }
  if (m.format !== EXTENSION_FORMAT) c.err("format", `must be "${EXTENSION_FORMAT}".`);
  if (m.version !== 1) c.err("version", "must be 1 (the format version). Your extension's own version goes in release.");
  if (!isId(m.id)) c.err("id", "must be lowercase letters, digits, dots and dashes, up to 64 characters (for example my-shelves).");
  if (!oneLine(m.name, 60)) c.err("name", "must be text of up to 60 characters.");
  if (!isSemver(m.release)) c.err("release", "must be a version like 1.0.0.");
  if (m.author !== undefined && !oneLine(m.author, 60)) c.err("author", "must be text of up to 60 characters.");
  if (m.description !== undefined && !isText(m.description, 400)) c.err("description", "must be text of up to 400 characters.");
  if (m.homepage !== undefined && !isHttps(m.homepage)) c.err("homepage", "must be an https address.");
  if (m.icon !== undefined && !ICON(m.icon)) c.err("icon", "must be an icon name or nine rows of nine . and # characters.");
  if (m.tags !== undefined) c.strings(m.tags, "tags", 24, 8);
  c.known(m, "", ["$schema", "format", "version", "id", "name", "release", "author", "description", "homepage", "icon", "tags", "permissions", "contributes"]);

  const perms = m.permissions ?? [];
  if (!Array.isArray(perms)) c.err("permissions", "must be a list.");
  else {
    perms.forEach((p, i) => { if (!(p in PERMISSIONS)) c.err(`permissions[${i}]`, `${JSON.stringify(p)} is not a permission. Known: ${Object.keys(PERMISSIONS).join(", ")}.`); });
    if (new Set(perms).size !== perms.length) c.err("permissions", "lists a permission twice.");
  }

  const k = m.contributes;
  if (!isObj(k)) c.err("contributes", "must be an object with at least one of " + CONTRIBUTIONS.join(", ") + ".");
  else {
    c.known(k, "contributes", CONTRIBUTIONS);
    if (!CONTRIBUTIONS.some((x) => Array.isArray(k[x]) && k[x].length)) c.err("contributes", "adds nothing. Add at least one shelf, rule, tool, launch option, command pack, link or panel.");

    list(c, k, "shelves", (s, at) => {
      c.known(s, at, ["id", "title", "hint", "icon", "query"]);
      if (!oneLine(s.title, 50)) c.err(`${at}.title`, "must be text of up to 50 characters.");
      if (s.hint !== undefined && !oneLine(s.hint, 90)) c.err(`${at}.hint`, "must be text of up to 90 characters.");
      if (s.icon !== undefined && !ICON(s.icon)) c.err(`${at}.icon`, "must be an icon name.");
      const q = s.query;
      if (!isObj(q)) return c.err(`${at}.query`, "must be an object (a saved search).");
      c.known(q, `${at}.query`, SHELF_FIELDS);
      if (q.text !== undefined && (typeof q.text !== "string" || q.text.length > 100)) c.err(`${at}.query.text`, "must be text of up to 100 characters.");
      for (const f of ["categories", "tags", "excludeTags"]) if (q[f] !== undefined) c.strings(q[f], `${at}.query.${f}`, 80, 10);
      if (q.sort !== undefined) c.oneOf(q.sort, `${at}.query.sort`, SORT_KEYS);
      if (q.ascending !== undefined && typeof q.ascending !== "boolean") c.err(`${at}.query.ascending`, "must be true or false.");
      if (q.frameworks !== undefined) { if (!Array.isArray(q.frameworks)) c.err(`${at}.query.frameworks`, "must be a list."); else q.frameworks.forEach((f, i) => c.oneOf(f, `${at}.query.frameworks[${i}]`, FRAMEWORK_KEYS)); }
      if (q.modType !== undefined) c.oneOf(q.modType, `${at}.query.modType`, MOD_TYPES);
      for (const f of ["endorsementsMin", "downloadsMax"]) if (q[f] !== undefined) c.count(q[f], `${at}.query.${f}`);
      for (const f of ["createdWithinDays"]) if (q[f] !== undefined) { c.count(q[f], `${at}.query.${f}`, 3650); if (q[f] === 0) c.err(`${at}.query.${f}`, "must be 1 or more."); }
    });

    list(c, k, "groupRules", (r, at) => {
      c.known(r, at, ["group", "match"]);
      if (!oneLine(r.group, 40)) c.err(`${at}.group`, "must be a group name of up to 40 characters.");
      if (!isObj(r.match)) return c.err(`${at}.match`, "must be an object.");
      c.known(r.match, `${at}.match`, MATCH_FIELDS);
      const mm = r.match;
      for (const f of ["name", "category", "author"]) if (mm[f] !== undefined) c.strings(mm[f], `${at}.match.${f}`, 80, 30);
      if (mm.modType !== undefined) { if (!Array.isArray(mm.modType)) c.err(`${at}.match.modType`, "must be a list."); else mm.modType.forEach((t, i) => c.oneOf(t, `${at}.match.modType[${i}]`, ALL_MOD_TYPES)); }
      if (mm.frameworks !== undefined) { if (!Array.isArray(mm.frameworks)) c.err(`${at}.match.frameworks`, "must be a list."); else mm.frameworks.forEach((f, i) => c.oneOf(f, `${at}.match.frameworks[${i}]`, FRAMEWORK_KEYS)); }
      if (mm.mods !== undefined) { if (!Array.isArray(mm.mods) || mm.mods.length > 200) c.err(`${at}.match.mods`, "must be a list of up to 200 Nexus mod ids."); else mm.mods.forEach((n, i) => { if (!Number.isInteger(n) || n <= 0) c.err(`${at}.match.mods[${i}]`, "must be a Nexus mod id."); }); }
      if (!MATCH_FIELDS.some((f) => Array.isArray(mm[f]) && mm[f].length)) c.err(`${at}.match`, "matches nothing. Give at least one of " + MATCH_FIELDS.join(", ") + ".");
    });

    list(c, k, "tools", (t, at) => {
      c.known(t, at, ["id", "name", "description", "path", "pick", "args", "download"]);
      if (!oneLine(t.name, 60)) c.err(`${at}.name`, "must be text of up to 60 characters.");
      if (t.description !== undefined && !isText(t.description, 200)) c.err(`${at}.description`, "must be text of up to 200 characters.");
      if ((t.path === undefined) === (t.pick === undefined)) c.err(at, "needs exactly one of path (inside the game folder) or pick (the user picks the program).");
      if (t.path !== undefined) {
        const parts = typeof t.path === "string" ? t.path.split(/[\\/]/) : [];
        if (!oneLine(t.path, 200) || /^[a-zA-Z]:|^[\\/]/.test(t.path) || parts.some((p) => p === ".." || p === "." || p === ""))
          c.err(`${at}.path`, "must be a path inside the game folder, like tools/redmod/bin/redMod.exe.");
        else if (!/\.exe$/i.test(t.path)) c.err(`${at}.path`, "must end in .exe.");
      }
      if (t.pick !== undefined) {
        if (!isObj(t.pick)) c.err(`${at}.pick`, "must be an object, for example { \"file\": \"WolvenKit.exe\" }.");
        else {
          c.known(t.pick, `${at}.pick`, ["title", "file"]);
          if (t.pick.title !== undefined && !oneLine(t.pick.title, 80)) c.err(`${at}.pick.title`, "must be text of up to 80 characters.");
          if (t.pick.file !== undefined && (!oneLine(t.pick.file, 80) || /[\\/]/.test(t.pick.file) || !/\.exe$/i.test(t.pick.file))) c.err(`${at}.pick.file`, "must be a file name ending in .exe, without folders.");
        }
      }
      if (t.args !== undefined) {
        if (!Array.isArray(t.args) || t.args.length > LIMITS.args) c.err(`${at}.args`, `must be a list of up to ${LIMITS.args} arguments.`);
        else t.args.forEach((a, i) => { if (!isArg(a)) c.err(`${at}.args[${i}]`, "must be text without quotes or line breaks."); });
      }
      if (t.download !== undefined && !isHttps(t.download)) c.err(`${at}.download`, "must be an https address.");
    });

    list(c, k, "launchOptions", (o, at) => {
      c.known(o, at, ["id", "name", "description", "args"]);
      if (!oneLine(o.name, 60)) c.err(`${at}.name`, "must be text of up to 60 characters.");
      if (o.description !== undefined && !isText(o.description, 200)) c.err(`${at}.description`, "must be text of up to 200 characters.");
      if (!Array.isArray(o.args) || o.args.length === 0 || o.args.length > 16) c.err(`${at}.args`, "must be a list of 1 to 16 game arguments.");
      else o.args.forEach((a, i) => { if (typeof a !== "string" || !/^[^\s"{}]{1,120}$/.test(a)) c.err(`${at}.args[${i}]`, "must be one argument without spaces, quotes or braces, like -skipStartScreen."); });
    });

    list(c, k, "commandPacks", (p, at) => {
      c.known(p, at, ["id", "title", "description", "mods", "commands"]);
      if (!oneLine(p.title, 60)) c.err(`${at}.title`, "must be text of up to 60 characters.");
      if (p.description !== undefined && !isText(p.description, 300)) c.err(`${at}.description`, "must be text of up to 300 characters.");
      if (p.mods !== undefined) { if (!Array.isArray(p.mods) || p.mods.length > 50) c.err(`${at}.mods`, "must be a list of up to 50 Nexus mod ids."); else p.mods.forEach((n, i) => { if (!Number.isInteger(n) || n <= 0) c.err(`${at}.mods[${i}]`, "must be a Nexus mod id."); }); }
      if (!Array.isArray(p.commands) || p.commands.length === 0 || p.commands.length > LIMITS.commands) return c.err(`${at}.commands`, `must be a list of 1 to ${LIMITS.commands} commands.`);
      p.commands.forEach((x, i) => {
        const here = `${at}.commands[${i}]`;
        if (!isObj(x)) return c.err(here, "must be an object with a command and an optional label.");
        c.known(x, here, ["command", "label"]);
        if (!oneLine(x.command, 300)) c.err(`${here}.command`, "must be one line of up to 300 characters.");
        if (x.label !== undefined && !oneLine(x.label, 80)) c.err(`${here}.label`, "must be text of up to 80 characters.");
      });
    });

    list(c, k, "links", (l, at) => {
      c.known(l, at, ["id", "title", "url"]);
      if (!oneLine(l.title, 60)) c.err(`${at}.title`, "must be text of up to 60 characters.");
      if (!isHttps(l.url)) c.err(`${at}.url`, "must be an https address.");
    });

    list(c, k, "panels", (p, at) => {
      c.known(p, at, ["id", "title", "icon", "html"]);
      if (!oneLine(p.title, 40)) c.err(`${at}.title`, "must be text of up to 40 characters.");
      if (p.icon !== undefined && !ICON(p.icon)) c.err(`${at}.icon`, "must be an icon name.");
      if (typeof p.html !== "string" || !p.html.trim()) c.err(`${at}.html`, "must be the panel's HTML as text.");
      else if (p.html.length > LIMITS.panelHtml) c.err(`${at}.html`, "is larger than 512 KB.");
      else if (/<(iframe|object|embed|base)\b/i.test(p.html)) c.warn(`${at}.html`, "uses a tag that does nothing in a panel (frames, objects and base are blocked).");
    });

    if (Array.isArray(perms) && perms.length && !(Array.isArray(k.panels) && k.panels.length)) c.warn("permissions", "only panels use permissions, and this extension has none.");
  }
  return { ok: c.errors.length === 0, errors: c.errors, warnings: c.warnings };
}

/** What the user approves on install. An update that needs anything not on this list asks again. */
export function approvalItems(m) {
  const k = (m && m.contributes) || {};
  return [
    ...(m.permissions ?? []).map((p) => `permission:${p}`),
    ...(k.tools ?? []).map((t) => `tool:${t.id}:${t.path ?? `pick:${t.pick?.file ?? ""}`}:${(t.args ?? []).join(" ")}`),
    ...(k.launchOptions ?? []).map((o) => `launch:${o.id}:${(o.args ?? []).join(" ")}`),
    ...(k.links ?? []).map((l) => `link:${l.url}`),
  ];
}

// ── themes (the format is the theme engine's, src/lib/theme.ts; these are the marketplace checks) ──

const TOKEN = /^--[a-z][a-z0-9-]*$/;
const NO_NETWORK = /url\s*\(|@import|expression\s*\(/i;

export function validateTheme(t) {
  const c = checker();
  if (!isObj(t)) { c.err("", "A theme is a JSON object."); return { ok: false, errors: c.errors, warnings: c.warnings }; }
  if (t.format !== THEME_FORMAT) c.err("format", `must be "${THEME_FORMAT}".`);
  if (t.version !== 1) c.err("version", "must be 1 (the format version).");
  if (!isId(t.id)) c.err("id", "must be lowercase letters, digits, dots and dashes.");
  if (!oneLine(t.name, 60)) c.err("name", "must be text of up to 60 characters.");
  if (t.release !== undefined && !isSemver(t.release)) c.err("release", "must be a version like 1.0.0.");
  if (t.author !== undefined && !oneLine(t.author, 60)) c.err("author", "must be text of up to 60 characters.");
  if (t.description !== undefined && !isText(t.description, 400)) c.err("description", "must be text of up to 400 characters.");
  if (t.scheme !== "dark" && t.scheme !== "light") c.err("scheme", 'must be "dark" or "light".');
  let any = 0;
  for (const part of ["tokens", "dark", "light"]) {
    const v = t[part];
    if (v === undefined) continue;
    if (!isObj(v)) { c.err(part, "must be an object of CSS custom properties."); continue; }
    for (const [key, val] of Object.entries(v)) {
      any++;
      if (!TOKEN.test(key)) c.err(`${part}.${key}`, "is not a CSS custom property (they start with --, like --bg).");
      if (typeof val !== "string" || !val.trim() || val.length > 300 || /[;{}<>]/.test(val)) c.err(`${part}.${key}`, "must be one CSS value, without ; { } < or >.");
      else if (NO_NETWORK.test(val)) c.err(`${part}.${key}`, "may not load anything (no url() or @import).");
    }
  }
  if (!any) c.err("tokens", "a theme needs colours in tokens, dark or light.");
  if (t.css !== undefined) {
    if (typeof t.css !== "string" || t.css.length > 64 * 1024) c.err("css", "must be text of up to 64 KB.");
    else if (NO_NETWORK.test(t.css)) c.err("css", "may not load anything (no url() or @import).");
  }
  return { ok: c.errors.length === 0, errors: c.errors, warnings: c.warnings };
}

// ── the marketplace index ──

const COLOR = /^#[0-9a-fA-F]{3,8}$/;
export const PREVIEW_KEYS = ["bg", "surface", "accent", "text", "chrome", "display"];

export function validateIndex(ix) {
  const c = checker();
  if (!isObj(ix)) { c.err("", "The index is a JSON object."); return { ok: false, errors: c.errors, warnings: c.warnings }; }
  if (ix.format !== MARKET_FORMAT) c.err("format", `must be "${MARKET_FORMAT}".`);
  if (ix.version !== 1) c.err("version", "must be 1.");
  c.known(ix, "", ["$schema", "format", "version", "updated", "themes", "extensions"]);
  const ids = new Set();
  for (const [key, kind] of [["themes", "theme"], ["extensions", "extension"]]) {
    const listv = ix[key];
    if (!Array.isArray(listv)) { c.err(key, "must be a list."); continue; }
    listv.forEach((e, i) => {
      const at = `${key}[${i}]`;
      if (!isObj(e)) return c.err(at, "must be an object.");
      c.known(e, at, ["id", "kind", "name", "author", "version", "description", "tags", "path", "updated", "preview", "icon", "screenshot", "permissions", "adds"]);
      if (!isId(e.id)) c.err(`${at}.id`, "must be an id.");
      else if (ids.has(`${kind}:${e.id}`)) c.err(`${at}.id`, `${e.id} is listed twice.`);
      else ids.add(`${kind}:${e.id}`);
      if (e.kind !== kind) c.err(`${at}.kind`, `must be "${kind}".`);
      if (!oneLine(e.name, 60)) c.err(`${at}.name`, "must be text of up to 60 characters.");
      if (!oneLine(e.author, 60)) c.err(`${at}.author`, "must be text of up to 60 characters.");
      if (!isSemver(e.version)) c.err(`${at}.version`, "must be a version like 1.0.0.");
      if (!isText(e.description, 200)) c.err(`${at}.description`, "must be text of up to 200 characters.");
      if (e.tags !== undefined) c.strings(e.tags, `${at}.tags`, 24, 8);
      if (typeof e.path !== "string" || !new RegExp(`^${key}/[a-z0-9][a-z0-9.-]*\\.json$`).test(e.path)) c.err(`${at}.path`, `must be a file in ${key}/, like ${key}/${e.id ?? "my-addon"}.json.`);
      if (typeof e.updated !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(e.updated) || Number.isNaN(Date.parse(e.updated))) c.err(`${at}.updated`, "must be a date like 2026-11-04.");
      if (e.preview !== undefined) {
        if (!isObj(e.preview)) c.err(`${at}.preview`, "must be an object of colours.");
        else {
          c.known(e.preview, `${at}.preview`, PREVIEW_KEYS);
          for (const [pk, pv] of Object.entries(e.preview)) if (typeof pv !== "string" || !COLOR.test(pv)) c.err(`${at}.preview.${pk}`, "must be a colour like #1a1917.");
        }
      }
      if (e.icon !== undefined && !ICON(e.icon)) c.err(`${at}.icon`, "must be an icon name or nine rows of nine . and # characters.");
      if (e.screenshot !== undefined && !(typeof e.screenshot === "string" && (/^[a-z0-9][a-z0-9./-]*\.(png|jpg|webp)$/.test(e.screenshot) && !e.screenshot.includes("..")))) c.err(`${at}.screenshot`, "must be an image file in the marketplace folder, like extensions/shots/my-addon.png.");
      if (e.permissions !== undefined) { if (!Array.isArray(e.permissions)) c.err(`${at}.permissions`, "must be a list."); else e.permissions.forEach((p, j) => { if (!(p in PERMISSIONS)) c.err(`${at}.permissions[${j}]`, `${JSON.stringify(p)} is not a permission.`); }); }
      if (e.adds !== undefined) c.strings(e.adds, `${at}.adds`, 60, 12);
    });
  }
  return { ok: c.errors.length === 0, errors: c.errors, warnings: c.warnings };
}

/** Plain words for what an extension adds, for cards and the install check. */
export function summarize(m) {
  const k = (m && m.contributes) || {};
  const n = (key) => (Array.isArray(k[key]) ? k[key].length : 0);
  const say = (count, one, many) => (count ? [`${count} ${count === 1 ? one : many}`] : []);
  return [
    ...say(n("shelves"), "Browse shelf", "Browse shelves"), ...say(n("groupRules"), "group rule", "group rules"),
    ...say(n("tools"), "tool", "tools"), ...say(n("launchOptions"), "launch option", "launch options"),
    ...say(n("commandPacks"), "command pack", "command packs"), ...say(n("links"), "link", "links"),
    ...say(n("panels"), "panel", "panels"),
  ];
}
