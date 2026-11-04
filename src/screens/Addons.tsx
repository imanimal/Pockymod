// Add-ons: themes and extensions from the marketplace (a folder in this project's GitHub repository), the ones you
// installed, their panels, and how to make and share your own. See docs/marketplace.md and docs/extensions.md.
import { useCallback, useEffect, useState } from "react";
import type { View } from "../App";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { Chip, Empty, Key, Module, Pill, SearchField, Switch, Tag } from "../ui/pocket";
import { toast } from "../ui/toast";
import { errorText, extensionsApi, pickProgram, pickTextFile, saveTextAs } from "../lib/api";
import { PERMISSIONS, summarize, type MarketEntry } from "../lib/addon-rules.mjs";
import {
  checkExtensionText, installExtension, newApprovals, reloadExtensions, removeExtension, setExtensionEnabled, setToolPath, useExtensions,
  type ExtensionManifest, type InstalledExtension,
} from "../lib/extensions";
import {
  checkThemeText, fetchPackage, installTheme, installedThemes, isNewer, loadIndex, marketVersions, packageFileName, recordVersion, removeTheme, screenshotUrl,
  type InstalledTheme, type LoadedIndex,
} from "../lib/market";
import { shortDate } from "../lib/text";
import { AddonGlyph, CommandList, ThemeSwatch, hostOf } from "./addons/parts";
import { ReviewDialog, type ReviewMode } from "./addons/ReviewDialog";
import { PanelHost } from "./addons/Panel";
import { MakeDialog, ShareDialog, openDocs } from "./addons/Share";

type Tab = "themes" | "extensions" | "installed" | `panel:${string}`;
type Sort = "updated" | "name" | "author";
type Review = { manifest: ExtensionManifest; text: string; mode: ReviewMode; fromMarket: boolean; warnings: string[] };

const TAB_KEY = "pm.addonsTab";
// a panel tab is never reopened at start: a misbehaving panel must not stand between the user and Installed
const readTab = (): Tab => { try { const t = localStorage.getItem(TAB_KEY); if (t === "themes" || t === "extensions" || t === "installed") return t as Tab; } catch { /* storage unavailable */ } return "themes"; };

export function Addons({ onNavigate }: { onNavigate?: (v: View) => void }) {
  const ext = useExtensions();
  const [tab, setTabState] = useState<Tab>(readTab);
  const setTab = (t: Tab) => { setTabState(t); try { localStorage.setItem(TAB_KEY, t); } catch { /* storage unavailable */ } };
  const [ix, setIx] = useState<LoadedIndex | null>(null);
  const [ixErr, setIxErr] = useState<string | null>(null);
  const [themes, setThemes] = useState<InstalledTheme[]>([]);
  const [versions, setVersions] = useState<Record<string, string>>({});
  const [text, setText] = useState("");
  const [sort, setSort] = useState<Sort>("updated");
  const [busy, setBusy] = useState<string | null>(null);
  const [review, setReview] = useState<Review | null>(null);
  const [share, setShare] = useState<{ kind: "theme" | "extension"; text: string } | null>(null);
  const [make, setMake] = useState(false);
  const [removing, setRemoving] = useState<InstalledExtension | null>(null);
  const [cmds, setCmds] = useState<InstalledExtension | null>(null);
  const [fileErrors, setFileErrors] = useState<{ kind: "theme" | "extension"; errors: string[] } | null>(null);

  const loadMarket = useCallback((fresh = false) => {
    setIxErr(null);
    loadIndex(fresh).then(setIx).catch((e) => setIxErr(errorText(e)));
  }, []);
  const loadLocal = useCallback(() => {
    installedThemes().then(setThemes).catch(() => {});
    marketVersions().then(setVersions).catch(() => {});
  }, []);
  useEffect(() => { loadMarket(); loadLocal(); void reloadExtensions(); }, [loadMarket, loadLocal]);

  const panels = ext.active.panels;
  useEffect(() => { if (tab.startsWith("panel:") && ext.loaded && !panels.some((p) => `panel:${p.extId}/${p.id}` === tab)) setTab("installed"); }, [tab, panels, ext.loaded]);

  // what is installed, and at which version
  const themeVersion = (id: string) => versions[`theme:${id}`] ?? themes.find((t) => t.id === id)?.release ?? (themes.some((t) => t.id === id) ? "" : undefined);
  const extOf = (id: string) => ext.list.find((e) => e.id === id);
  const installedVersion = (e: MarketEntry) => (e.kind === "theme" ? themeVersion(e.id) : extOf(e.id)?.manifest?.release ?? (extOf(e.id) ? "" : undefined));
  const updates = [...(ix?.index.themes ?? []), ...(ix?.index.extensions ?? [])].filter((e) => isNewer(e, installedVersion(e)));

  const run = async (key: string, f: () => Promise<void>) => { setBusy(key); try { await f(); } catch (e) { toast(errorText(e), true); } finally { setBusy(null); loadLocal(); } };

  const installEntry = (e: MarketEntry) => run(`${e.kind}:${e.id}`, async () => {
    const pkg = await fetchPackage(e);
    if (e.kind === "theme") {
      const was = themeVersion(e.id);
      const r = await installTheme(pkg, e);
      toast(was !== undefined ? `${r.name} is updated to ${e.version}.` : `${r.name} is installed. Pick it in Settings, under Look.`);
      return;
    }
    const c = checkExtensionText(pkg);
    if (!c.manifest) throw new Error(`${e.name} does not check out: ${c.errors[0]}`);
    const have = extOf(e.id);
    if (have && have.manifest && newApprovals(c.manifest, have.state).length === 0 && have.state.approvedRelease) {
      await installExtension(pkg, { approve: false, fromMarket: true });
      await recordVersion("extension", e.id, c.manifest.release);
      toast(`${c.manifest.name} is updated to ${c.manifest.release}.`);
      return;
    }
    setReview({ manifest: c.manifest, text: pkg, mode: have ? "update" : "install", fromMarket: true, warnings: c.warnings });
  });

  const fromFile = () => run("file", async () => {
    const f = await pickTextFile("Install a theme or extension", ["json"]);
    if (!f) return;
    let format: unknown;
    try { format = (JSON.parse(f.text) as { format?: unknown }).format; } catch { throw new Error("That file is not valid JSON."); }
    if (format === "pocket-mods.theme") {
      const c = checkThemeText(f.text);
      if (!c.pkg) { setFileErrors({ kind: "theme", errors: c.errors }); return; }
      const r = await installTheme(f.text);
      toast(`${r.name} is installed. Pick it in Settings, under Look.`);
      setTab("installed");
    } else if (format === "pocket-mods.extension") {
      const c = checkExtensionText(f.text);
      if (!c.manifest) { setFileErrors({ kind: "extension", errors: c.errors }); return; }
      setReview({ manifest: c.manifest, text: f.text, mode: extOf(c.manifest.id) ? "update" : "install", fromMarket: false, warnings: c.warnings });
    } else throw new Error("That is not a Pocket Mods theme or extension (its format field says what it is).");
  });

  const match = (e: MarketEntry) => {
    const t = text.trim().toLowerCase();
    return !t || `${e.name} ${e.author} ${e.description} ${(e.tags ?? []).join(" ")}`.toLowerCase().includes(t);
  };
  const order = (a: MarketEntry, b: MarketEntry) => sort === "name" ? a.name.localeCompare(b.name) : sort === "author" ? a.author.localeCompare(b.author) || a.name.localeCompare(b.name) : b.updated.localeCompare(a.updated);
  const list = (k: "themes" | "extensions") => (ix?.index[k] ?? []).filter(match).sort(order);
  const installedCount = themes.length + ext.list.length;
  const panel = panels.find((p) => `panel:${p.extId}/${p.id}` === tab);

  return (
    <div className={`stack addons${panel ? " with-panel" : ""}`}>
      <div className="browse-top">
        <SearchField value={text} onChange={setText} placeholder="Search themes and extensions" />
        <select value={sort} onChange={(e) => setSort(e.target.value as Sort)} aria-label="Sort">
          <option value="updated">Recently updated</option><option value="name">Name</option><option value="author">Author</option>
        </select>
        <Key size="sm" icon="file" onClick={fromFile} disabled={busy === "file"}>Install from file</Key>
        <Key size="sm" icon="plus" variant="ghost" onClick={() => setMake(true)}>Make your own</Key>
      </div>

      <div className="vtabs" role="tablist" aria-label="Add-ons">
        <Chip on={tab === "themes"} count={ix?.index.themes.length} onClick={() => setTab("themes")}>Themes</Chip>
        <Chip on={tab === "extensions"} count={ix?.index.extensions.length} onClick={() => setTab("extensions")}>Extensions</Chip>
        <Chip on={tab === "installed"} count={installedCount || undefined} onClick={() => setTab("installed")}>{updates.length ? `Installed · ${updates.length} ${updates.length === 1 ? "update" : "updates"}` : "Installed"}</Chip>
        {panels.length > 0 && <span className="vtab-sep" aria-hidden="true" />}
        {panels.map((p) => (
          <Chip key={`${p.extId}/${p.id}`} on={tab === `panel:${p.extId}/${p.id}`} onClick={() => setTab(`panel:${p.extId}/${p.id}`)} title={`A panel from ${p.extName}`}>
            <AddonGlyph icon={p.icon} size={9} /> {p.title}
          </Chip>
        ))}
      </div>

      {!panel && ixErr && <div className="note bad">The marketplace could not be loaded: {ixErr} <Key size="sm" variant="ghost" icon="refresh" onClick={() => loadMarket(true)}>Try again</Key></div>}
      {!panel && ix?.source === "offline" && (tab === "themes" || tab === "extensions") && (
        <div className="note warn">The marketplace could not be reached, so this is the copy saved the last time it was. <Key size="sm" variant="ghost" icon="refresh" onClick={() => loadMarket(true)}>Try again</Key></div>
      )}
      {!panel && ix?.source === "bundled" && (tab === "themes" || tab === "extensions") && (
        <div className="note">This is the marketplace that came with this version of the app: the online one is not published yet or could not be reached. Its pictures may be missing. <Key size="sm" variant="ghost" icon="refresh" onClick={() => loadMarket(true)}>Try again</Key></div>
      )}

      {panel ? <PanelHost key={`${panel.extId}/${panel.id}/${panel.release}`} panel={panel} />
        : tab === "installed" ? (
          <Installed ext={ext.list} themes={themes} versions={versions} index={ix?.index ?? null} updates={updates} busy={busy}
            onUpdate={installEntry} onShare={setShare} onRemoveExt={setRemoving} onCommands={setCmds} onNavigate={onNavigate}
            onOpenPanel={(id) => setTab(`panel:${id}`)} onReview={(x) => x.manifest && setReview({ manifest: x.manifest, text: x.text, mode: "review", fromMarket: x.state.fromMarket, warnings: x.warnings })}
            onRemoveTheme={(t) => run(`theme:${t.id}`, async () => { await removeTheme(t.id); toast(`${t.pkg.name} is removed.`); })}
            onBrowse={() => setTab("extensions")} />
        ) : (
          <MarketGrid kind={tab === "themes" ? "theme" : "extension"} entries={list(tab === "themes" ? "themes" : "extensions")} loaded={!!ix || !!ixErr}
            installedVersion={installedVersion} busy={busy} onInstall={installEntry} searching={!!text.trim()} onMake={() => setMake(true)}
            onRemove={(e) => e.kind === "theme" ? run(`theme:${e.id}`, async () => { await removeTheme(e.id); toast(`${e.name} is removed.`); }) : setRemoving(extOf(e.id) ?? null)} />
        )}

      {review && <ReviewDialog manifest={review.manifest} text={review.text} mode={review.mode} fromMarket={review.fromMarket} warnings={review.warnings}
        state={extOf(review.manifest.id)?.state} onClose={() => setReview(null)} onDone={() => { loadLocal(); setTab("installed"); }} />}
      {share && <ShareDialog kind={share.kind} text={share.text} onClose={() => setShare(null)} />}
      {make && <MakeDialog onClose={() => setMake(false)} />}
      {fileErrors && (
        <Dialog icon="warn" title="This file needs fixing" count={`${fileErrors.errors.length} ${fileErrors.errors.length === 1 ? "problem" : "problems"}`} band="bad" onClose={() => setFileErrors(null)}>
          <p>Nothing was installed. Each line names the field to fix in the {fileErrors.kind} file:</p>
          <ul className="ext-errs">{fileErrors.errors.slice(0, 20).map((e) => <li key={e} className="mono">{e}</li>)}</ul>
          <p className="dim" style={{ fontSize: ".86em" }}>Fix them in a text editor, save, and use Install from file again.{" "}
            <button type="button" className="linkish" onClick={() => openDocs(fileErrors.kind === "theme" ? "themes" : "extensions")}>The {fileErrors.kind} format</button></p>
          <div className="actions" style={{ justifyContent: "flex-end" }}><Key variant="ghost" onClick={() => setFileErrors(null)}>Close</Key></div>
        </Dialog>
      )}
      {removing && (
        <Dialog icon="trash" title="Remove this extension?" count={removing.manifest?.name ?? removing.id} onClose={() => setRemoving(null)}>
          <p>{removing.manifest?.name ?? removing.id} and what it adds go away. Notes it kept are deleted too. Mods it sorted into groups stay where they are.</p>
          <div className="actions" style={{ justifyContent: "flex-end" }}>
            <Key variant="ghost" onClick={() => setRemoving(null)}>Cancel</Key>
            <Key icon="trash" onClick={() => { const x = removing; setRemoving(null); void run(`extension:${x.id}`, async () => { await removeExtension(x.id); await recordVersion("extension", x.id, null); toast(`${x.manifest?.name ?? x.id} is removed.`); }); }}>Remove</Key>
          </div>
        </Dialog>
      )}
      {cmds?.manifest && (
        <Dialog icon="file" title="Console commands" count={cmds.manifest.name} onClose={() => setCmds(null)}>
          <p className="dim">Type these in the Cyber Engine Tweaks console in game (the key you set for it, often ~).</p>
          {(cmds.manifest.contributes.commandPacks ?? []).map((p) => (
            <div key={p.id} className="stack" style={{ gap: ".3em" }}>
              <div className="sub-h">{p.title}</div>
              {p.description && <p className="dim" style={{ fontSize: ".86em" }}>{p.description}</p>}
              <CommandList commands={p.commands} />
            </div>
          ))}
        </Dialog>
      )}
    </div>
  );
}

function MarketGrid({ kind, entries, loaded, installedVersion, busy, onInstall, onRemove, searching, onMake }: {
  kind: "theme" | "extension"; entries: MarketEntry[]; loaded: boolean; installedVersion: (e: MarketEntry) => string | undefined; busy: string | null;
  onInstall: (e: MarketEntry) => void; onRemove: (e: MarketEntry) => void; searching: boolean; onMake: () => void;
}) {
  if (!loaded) return <div className="agrid">{Array.from({ length: 4 }, (_, i) => <span key={i} className="acard panel skel" />)}</div>;
  if (!entries.length) return (
    <Module icon="addons" title={kind === "theme" ? "Themes" : "Extensions"}>
      <Empty icon="addons" title={searching ? "Nothing matches" : `No ${kind}s yet`} actions={<Key icon="plus" onClick={onMake}>Make your own</Key>}>
        {searching ? "Try other words." : `Be the first to share a ${kind}.`}
      </Empty>
    </Module>
  );
  return (
    <>
      <p className="dim addons-hint">{kind === "theme"
        ? "Themes change the app's colours. After installing one, pick it in Settings, under Look."
        : "Extensions add shelves, group rules, tools, launch options, console commands, links and panels. You see what each can do before it is installed."}</p>
      <div className="agrid">
        {entries.map((e) => {
          const have = installedVersion(e);
          const upd = isNewer(e, have);
          const working = busy === `${e.kind}:${e.id}`;
          return (
            <article key={e.id} className="acard panel">
              <header className="ph"><span className="pt"><Icon name={e.kind === "theme" ? "sun" : "addons"} size={9} />{e.kind === "theme" ? "Theme" : "Extension"}</span><span className="sp" /><span className="cnt">v{e.version}</span></header>
              <div className={`acard-pre${e.kind === "extension" ? " ext" : ""}`}>
                {e.kind === "theme" ? <ThemeSwatch preview={e.preview} />
                  : e.screenshot ? <img src={screenshotUrl(e.screenshot)} alt="" loading="lazy" />
                    : <span className="acard-glyph"><AddonGlyph icon={e.icon} size={54} /></span>}
                {have !== undefined && <span className="acard-badge"><Pill tone={upd ? "soon" : "ok"}>{upd ? "Update" : "Installed"}</Pill></span>}
              </div>
              <div className="acard-b">
                <b className="acard-t">{e.name}</b>
                <span className="m-meta"><span>{e.author}</span><span>{shortDate(`${e.updated}T12:00:00Z`)}</span></span>
                <span className="acard-s">{e.description}</span>
                {e.kind === "extension" && (
                  <span className="acard-can">
                    {e.adds?.length ? <span><Icon name="plus" size={9} /> {e.adds.join(" · ")}</span> : null}
                    <span><Icon name="key" size={9} /> {e.permissions?.length ? `Can: ${e.permissions.map((p) => (PERMISSIONS[p] ? PERMISSIONS[p].title[0].toLowerCase() + PERMISSIONS[p].title.slice(1) : p)).join(", ")}` : "Needs no permissions"}</span>
                  </span>
                )}
                {(e.tags?.length ?? 0) > 0 && <span className="m-tags">{e.tags!.map((t) => <Tag key={t}>{t}</Tag>)}</span>}
                <span className="acard-f">
                  {have === undefined ? <Key size="sm" icon="install" disabled={working} onClick={() => onInstall(e)}>{working ? "Installing" : "Install"}</Key>
                    : upd ? <Key size="sm" icon="downloads" disabled={working} onClick={() => onInstall(e)}>{working ? "Updating" : `Update to ${e.version}`}</Key>
                      : <span className="dim acard-have"><Icon name="check" size={9} /> v{have || e.version} installed</span>}
                  <span className="sp" />
                  {have !== undefined && <Key size="sm" variant="ghost" icon="trash" disabled={working} onClick={() => onRemove(e)} aria-label={`Remove ${e.name}`} title="Remove" />}
                </span>
              </div>
            </article>
          );
        })}
      </div>
    </>
  );
}

const STATUS: Record<InstalledExtension["status"], { tone?: "ok" | "soon" | "over"; label: string }> = {
  on: { tone: "ok", label: "On" }, off: { label: "Off" }, review: { tone: "soon", label: "Needs your OK" }, broken: { tone: "over", label: "Broken" },
};

function Installed({ ext, themes, versions, index, updates, busy, onUpdate, onShare, onRemoveExt, onRemoveTheme, onReview, onCommands, onOpenPanel, onNavigate, onBrowse }: {
  ext: InstalledExtension[]; themes: InstalledTheme[]; versions: Record<string, string>; index: LoadedIndex["index"] | null; updates: MarketEntry[]; busy: string | null;
  onUpdate: (e: MarketEntry) => Promise<void>; onShare: (s: { kind: "theme" | "extension"; text: string }) => void; onRemoveExt: (x: InstalledExtension) => void;
  onRemoveTheme: (t: InstalledTheme) => void; onReview: (x: InstalledExtension) => void; onCommands: (x: InstalledExtension) => void; onOpenPanel: (id: string) => void;
  onNavigate?: (v: View) => void; onBrowse: () => void;
}) {
  const [open, setOpen] = useState<Set<string>>(new Set());
  const entry = (kind: "theme" | "extension", id: string) => (kind === "theme" ? index?.themes : index?.extensions)?.find((e) => e.id === id);
  const updateFor = (kind: "theme" | "extension", id: string) => updates.find((u) => u.kind === kind && u.id === id);
  const exportText = (kind: "theme" | "extension", id: string, text: string) => saveTextAs(packageFileName(kind, id), text, ["json"]).catch((e) => toast(errorText(e), true));
  const toggleOpen = (id: string) => setOpen((s) => { const n = new Set(s); n.has(id) ? n.delete(id) : n.add(id); return n; });

  if (!ext.length && !themes.length) return (
    <Module icon="addons" title="Installed">
      <Empty icon="addons" title="Nothing installed yet" actions={<Key icon="addons" onClick={onBrowse}>See extensions</Key>}>
        Get themes and extensions from the marketplace, or install one from a file.
      </Empty>
    </Module>
  );

  return (
    <div className="stack addons-inst">
      {updates.length > 0 && (
        <Module icon="downloads" title="Updates" count={updates.length} band="hot"
          tools={<Key size="sm" icon="downloads" disabled={!!busy} onClick={async () => { for (const u of updates) await onUpdate(u); }}>Update all</Key>}>
          <div className="rows">
            {updates.map((u) => (
              <div key={`${u.kind}:${u.id}`} className="row" style={{ cursor: "default" }}>
                <span className="m-main"><span className="m-t">{u.name}</span><span className="m-meta"><span>{u.kind === "theme" ? "Theme" : "Extension"}</span><span>to v{u.version}</span><span>{shortDate(`${u.updated}T12:00:00Z`)}</span></span></span>
                <Key size="sm" icon="downloads" disabled={busy === `${u.kind}:${u.id}`} onClick={() => onUpdate(u)}>Update</Key>
              </div>
            ))}
          </div>
        </Module>
      )}

      <Module icon="addons" title="Extensions" count={ext.length ? `${ext.filter((x) => x.status === "on").length} of ${ext.length} on` : "none"} bodyClass="list">
        {!ext.length && <p className="dim" style={{ padding: ".6em" }}>No extensions yet.</p>}
        <div className="rows">
          {ext.map((x) => {
            const m = x.manifest;
            const st = STATUS[x.status];
            const upd = updateFor("extension", x.id);
            const isOpen = open.has(x.id);
            const k = m?.contributes;
            return (
              <div key={x.id} className={`ext-row${x.status === "on" ? "" : " off"}`}>
                <div className="ext-row-h">
                  <span className="ext-glyph sm"><AddonGlyph icon={m?.icon} size={27} /></span>
                  <span className="m-main">
                    <span className="m-t">{m?.name ?? x.id}</span>
                    <span className="m-meta">
                      {m && <span>v{m.release}</span>}<span>{m?.author ?? "Unknown author"}</span>
                      <span><Pill tone={st.tone}>{st.label}</Pill></span>
                      {upd && <span><Pill tone="soon">v{upd.version} available</Pill></span>}
                      {x.state.fromMarket || versions[`extension:${x.id}`] ? <span>Marketplace</span> : <span>From a file</span>}
                    </span>
                    {m && <span className="ext-sum">{summarize(m).join(" · ")}</span>}
                  </span>
                  <span className="ext-row-k">
                    {x.status === "review" && <Key size="sm" icon="check" onClick={() => onReview(x)}>Review</Key>}
                    {(x.status === "on" || x.status === "off") && <Switch checked={x.status === "on"} disabled={busy !== null}
                      onChange={(v) => setExtensionEnabled(x.id, v).then(() => toast(`${m?.name} is ${v ? "on" : "off"}.`)).catch((e) => toast(errorText(e), true))}>
                      <span className="sr-only">{m?.name} on</span></Switch>}
                    <Key size="sm" variant="ghost" icon="chevron" className={isOpen ? "rot-key" : ""} onClick={() => toggleOpen(x.id)} aria-expanded={isOpen}>Details</Key>
                  </span>
                </div>
                {x.status === "broken" && <div className="note bad ext-errs-n">{x.errors.slice(0, 4).map((e) => <div key={e} className="mono">{e}</div>)}</div>}
                {x.status === "review" && <p className="dim ext-fine">This release changed what it needs since you allowed it. It does nothing until you review it.</p>}
                {isOpen && m && k && (
                  <div className="ext-more">
                    {m.description && <p>{m.description}</p>}
                    {(k.panels ?? []).length > 0 && x.status === "on" && <div className="actions">{k.panels!.map((p) => <Key key={p.id} size="sm" icon="addons" onClick={() => onOpenPanel(`${x.id}/${p.id}`)}>Open {p.title}</Key>)}</div>}
                    {(k.tools ?? []).length > 0 && <>
                      <div className="sub-h">Tools, in the Play menu</div>
                      {k.tools!.map((t) => (
                        <div key={t.id} className="ext-pick">
                          <span className="m-main"><span className="m-t">{t.name}</span>
                            <span className="dim mono ext-path">{t.path ? `Game folder\\${t.path.replace(/\//g, "\\")}` : x.state.toolPaths[t.id] ?? "Not chosen yet"}</span></span>
                          {t.pick && <Key size="sm" icon="folder" onClick={async () => { const p = await pickProgram(t.pick?.title ?? `Find ${t.pick?.file ?? t.name}`); if (p) setToolPath(x.id, t.id, p).then(() => toast(`${t.name} is set.`)).catch((e) => toast(errorText(e), true)); }}>
                            {x.state.toolPaths[t.id] ? "Change" : `Choose ${t.pick.file ?? "program"}`}</Key>}
                          {t.download && <Key size="sm" variant="ghost" icon="web" disabled={x.status !== "on"} title={t.download} onClick={() => extensionsApi.openLink(x.id, `tool:${t.id}`).catch((e) => { if (!("__TAURI_INTERNALS__" in window)) window.open(t.download, "_blank", "noopener,noreferrer"); else toast(errorText(e), true); })}>Get it</Key>}
                        </div>
                      ))}
                    </>}
                    {(k.launchOptions ?? []).length > 0 && <>
                      <div className="sub-h">Launch options, in the Play menu</div>
                      <ul className="ext-list">{k.launchOptions!.map((o) => <li key={o.id}><b>{o.name}</b> <code>{o.args.join(" ")}</code>{o.description && <span className="dim"> · {o.description}</span>}</li>)}</ul>
                    </>}
                    {(k.links ?? []).length > 0 && <>
                      <div className="sub-h">Links</div>
                      <div className="actions">{k.links!.map((l) => <Key key={l.id} size="sm" variant="ghost" icon="web" disabled={x.status !== "on"} title={l.url}
                        onClick={() => extensionsApi.openLink(x.id, l.id).catch((e) => { if (!("__TAURI_INTERNALS__" in window)) window.open(l.url, "_blank", "noopener,noreferrer"); else toast(errorText(e), true); })}>{l.title} <span className="dim">· {hostOf(l.url)}</span></Key>)}</div>
                    </>}
                    {(k.commandPacks ?? []).length > 0 && <div className="actions"><Key size="sm" icon="file" onClick={() => onCommands(x)}>Console commands ({k.commandPacks!.reduce((a, p) => a + p.commands.length, 0)})</Key></div>}
                    {(k.shelves ?? []).length > 0 && <p className="dim ext-fine">Shelves show on the Browse front page. {(k.groupRules ?? []).length > 0 && "Group rules apply when you use Sort into groups in Library."}</p>}
                    {(k.shelves ?? []).length === 0 && (k.groupRules ?? []).length > 0 && <p className="dim ext-fine">Group rules apply when you use Sort into groups in Library.</p>}
                  </div>
                )}
                <div className="actions ext-row-f">
                  {upd && <Key size="sm" icon="downloads" disabled={busy === `extension:${x.id}`} onClick={() => onUpdate(upd)}>Update to {upd.version}</Key>}
                  {m && <Key size="sm" variant="ghost" icon="share" onClick={() => onShare({ kind: "extension", text: x.text })}>Share</Key>}
                  {m && <Key size="sm" variant="ghost" icon="file" onClick={() => void exportText("extension", x.id, x.text)}>Save as file</Key>}
                  <Key size="sm" variant="ghost" icon="trash" disabled={x.id === "unknown"} onClick={() => onRemoveExt(x)}>Remove</Key>
                  {!entry("extension", x.id) && m && <span className="dim ext-fine">Not on the marketplace</span>}
                </div>
              </div>
            );
          })}
        </div>
      </Module>

      <Module icon="sun" title="Themes" count={themes.length || "none"} bodyClass="list"
        tools={onNavigate && <Key size="sm" variant="ghost" icon="settings" onClick={() => onNavigate("settings")}>Pick one in Settings</Key>}>
        {!themes.length && <p className="dim" style={{ padding: ".6em" }}>No themes yet. The built-in ones are in Settings, under Look.</p>}
        <div className="rows">
          {themes.map((t) => {
            const upd = updateFor("theme", t.id);
            const v = versions[`theme:${t.id}`] ?? t.release;
            return (
              <div key={t.id} className="row ext-theme" style={{ cursor: "default" }}>
                <ThemeSwatch pkg={t.pkg} compact />
                <span className="m-main">
                  <span className="m-t">{t.pkg.name}</span>
                  <span className="m-meta">{v && <span>v{v}</span>}<span>{t.pkg.author ?? "Unknown author"}</span>{upd && <span><Pill tone="soon">v{upd.version} available</Pill></span>}</span>
                  {t.pkg.description && <span className="ext-sum">{t.pkg.description}</span>}
                </span>
                <span className="actions">
                  {upd && <Key size="sm" icon="downloads" disabled={busy === `theme:${t.id}`} onClick={() => onUpdate(upd)}>Update</Key>}
                  <Key size="sm" variant="ghost" icon="share" onClick={() => onShare({ kind: "theme", text: t.text })}>Share</Key>
                  <Key size="sm" variant="ghost" icon="file" onClick={() => void exportText("theme", t.id, t.text)} aria-label={`Save ${t.pkg.name} as a file`} title="Save as file" />
                  <Key size="sm" variant="ghost" icon="trash" disabled={busy === `theme:${t.id}`} onClick={() => onRemoveTheme(t)} aria-label={`Remove ${t.pkg.name}`} title="Remove" />
                </span>
              </div>
            );
          })}
        </div>
      </Module>

      <p className="dim addons-hint">Made one yourself? Use Share on it. <button type="button" className="linkish" onClick={() => openDocs("marketplace")}>How the marketplace works</button></p>
    </div>
  );
}
