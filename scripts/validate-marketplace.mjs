#!/usr/bin/env node
// Checks the marketplace folder: marketplace/index.json and every package it lists. Plain node, no packages.
//   node scripts/validate-marketplace.mjs            the repository's marketplace/ folder
//   node scripts/validate-marketplace.mjs <folder>    another folder laid out the same way
//   node scripts/validate-marketplace.mjs --file x.json   one theme or extension file on its own
// It uses the same rules as the app (src/lib/addon-rules.mjs) and exits with 1 when anything is wrong.
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { LIMITS, PERMISSIONS, validateExtension, validateIndex, validateTheme } from "../src/lib/addon-rules.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const problems = [];
const notes = [];
const bad = (where, msg) => problems.push(`${where}: ${msg}`);

function readJson(path, where) {
  let text;
  try { text = readFileSync(path, "utf8"); } catch { bad(where, "cannot be read."); return null; }
  if (text.length > LIMITS.manifest * 4) { bad(where, "is larger than 4 MB."); return null; }
  if (/\b(?:[A-Za-z0-9+/]{40,}={0,2})\b/.test(text.replace(/"html":\s*"(?:[^"\\]|\\.)*"/g, '""')) && /api[_-]?key|token|secret/i.test(text))
    notes.push(`${where}: has a long string near the word key, token or secret. Check that it holds no keys.`);
  try { return JSON.parse(text); } catch (e) { bad(where, `is not valid JSON (${e.message}).`); return null; }
}

function checkPackage(pkg, where) {
  const r = pkg?.format === "pocket-mods.theme" ? validateTheme(pkg) : pkg?.format === "pocket-mods.extension" ? validateExtension(pkg)
    : { ok: false, errors: ['format must be "pocket-mods.theme" or "pocket-mods.extension".'], warnings: [] };
  for (const e of r.errors) bad(where, e);
  for (const w of r.warnings) notes.push(`${where}: ${w}`);
  return r.ok;
}

if (args[0] === "--file") {
  for (const f of args.slice(1)) { const pkg = readJson(f, f); if (pkg) checkPackage(pkg, f); }
} else {
  const folder = resolve(args[0] ?? join(repo, "marketplace"));
  const at = (p) => relative(process.cwd(), join(folder, p)) || p;
  const ix = readJson(join(folder, "index.json"), at("index.json"));
  if (ix) {
    for (const e of validateIndex(ix).errors) bad(at("index.json"), e);
    const listed = new Set();
    const ids = new Map();
    for (const [key, kind] of [["themes", "theme"], ["extensions", "extension"]]) {
      for (const [i, e] of (Array.isArray(ix[key]) ? ix[key] : []).entries()) {
        const where = `${at("index.json")} ${key}[${i}] (${e?.id ?? "no id"})`;
        if (!e || typeof e.path !== "string") continue;
        if (ids.has(e.id)) bad(where, `the id ${e.id} is also used by ${ids.get(e.id)}. Ids are unique across themes and extensions.`);
        else ids.set(e.id, `${key}[${i}]`);
        const file = join(folder, e.path);
        listed.add(resolve(file));
        if (!existsSync(file)) { bad(where, `${e.path} does not exist.`); continue; }
        const pkg = readJson(file, at(e.path));
        if (!pkg) continue;
        if (!checkPackage(pkg, at(e.path))) continue;
        if (pkg.format !== `pocket-mods.${kind}`) bad(where, `${e.path} is not a ${kind}.`);
        if (pkg.id !== e.id) bad(where, `the package's id is ${pkg.id}, the index says ${e.id}.`);
        if (pkg.name !== e.name) notes.push(`${where}: the index calls it "${e.name}", the package "${pkg.name}".`);
        const release = kind === "extension" ? pkg.release : pkg.release ?? e.version;
        if (release !== e.version) bad(where, `the index says version ${e.version}, the package's release is ${release}. Raise both together.`);
        if (kind === "extension") {
          const want = [...(pkg.permissions ?? [])].sort().join(",");
          const have = [...(e.permissions ?? [])].sort().join(",");
          if (want !== have) bad(where, `permissions in the index (${have || "none"}) must match the package (${want || "none"}), so the card says what it can do.`);
          for (const p of e.permissions ?? []) if (!(p in PERMISSIONS)) bad(where, `${p} is not a known permission.`);
        }
        if (e.screenshot && !existsSync(join(folder, e.screenshot))) bad(where, `the screenshot ${e.screenshot} does not exist.`);
        else if (e.screenshot && statSync(join(folder, e.screenshot)).size > 1024 * 1024) bad(where, "the screenshot is larger than 1 MB.");
      }
    }
    // every package in the folders is listed, so nothing is shipped by accident
    for (const sub of ["themes", "extensions"]) {
      const dir = join(folder, sub);
      if (!existsSync(dir)) continue;
      for (const f of readdirSync(dir)) if (f.endsWith(".json") && !listed.has(resolve(join(dir, f)))) bad(at(`${sub}/${f}`), "is not listed in index.json.");
    }
    const count = (ix.themes?.length ?? 0) + (ix.extensions?.length ?? 0);
    if (!problems.length) notes.unshift(`${count} packages checked (${ix.themes?.length ?? 0} themes, ${ix.extensions?.length ?? 0} extensions).`);
  }
}

for (const n of notes) console.log(`note  ${n}`);
for (const p of problems) console.log(`error ${p}`);
if (problems.length) { console.log(`\n${problems.length} ${problems.length === 1 ? "problem" : "problems"}. See docs/marketplace.md for the rules.`); process.exit(1); }
console.log(args[0] === "--file" ? "The file checks out." : "The marketplace checks out.");
