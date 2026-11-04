// A brand-new mod. The Upload API cannot make a mod page, so this is a guided trip: check and pack here, fill the
// website's "add a mod" form in the in-app Nexus window with a checklist beside it, then bring the new page into
// My mods.
import { useEffect, useState, type ReactNode } from "react";
import { Icon } from "../../ui/Icon";
import { Key, Module, Pill, Tag } from "../../ui/pocket";
import { authoring, browse, errorText, settings, type MyMods } from "../../lib/api";
import { shortDate } from "../../lib/text";
import { toast } from "../../ui/toast";
import { CheckModule, PackModule } from "./Pack";
import { copy, fileName, openNexus, type Pack } from "./shared";

interface Draft { summary: string; category: string; done: string[]; startedAt: number }
const DRAFT_KEY = "creator.draft";
const blank = (): Draft => ({ summary: "", category: "", done: [], startedAt: Math.floor(Date.now() / 1000) });

export function NewMod({ pack, mine, onRefresh, onOpenMine }: { pack: Pack; mine: MyMods | null; onRefresh: () => Promise<void>; onOpenMine: () => void }) {
  const [draft, setDraft] = useState<Draft>(blank);
  const [cats, setCats] = useState<string[]>([]);
  const [link, setLink] = useState("");
  const [looking, setLooking] = useState(false);
  useEffect(() => {
    settings.get(DRAFT_KEY).then((v) => { if (v) try { setDraft({ ...blank(), ...JSON.parse(v) }); } catch { /* start over */ } }).catch(() => {});
    browse.search({ categories: [], tags: [], excludeTags: [], sort: "downloads", ascending: false, includeAdult: false, frameworks: [], offset: 0, count: 1 },
      { showHidden: false, downloaded: null, skipCounts: true })
      .then((r) => setCats(r.facets.categories.map(([c]) => c).sort((a, b) => a.localeCompare(b)))).catch(() => {});
  }, []);
  const save = (d: Draft) => { setDraft(d); settings.set(DRAFT_KEY, JSON.stringify(d)).catch(() => {}); };
  const tick = (id: string, on: boolean) => save({ ...draft, done: on ? [...new Set([...draft.done, id])] : draft.done.filter((x) => x !== id) });

  const needs = pack.check?.needs ?? [];
  const zip = pack.zip;
  const newest = mine ? [...mine.mods].sort((a, b) => b.createdAt.localeCompare(a.createdAt)).slice(0, 3) : [];
  const fresh = newest.filter((m) => Date.parse(m.createdAt) / 1000 >= draft.startedAt - 86400);
  const step = !zip ? 1 : draft.done.length < ITEMS.length ? 2 : 3;

  const addLink = async () => {
    if (!mine) return;
    try { await authoring.pin(mine.author.memberId, link); setLink(""); await onRefresh(); toast("Added to My mods."); onOpenMine(); } catch (e) { toast(errorText(e), true); }
  };

  const row = (id: string, title: string, body: ReactNode, tools?: ReactNode) => (
    <div className={`cr-check${draft.done.includes(id) ? " done" : ""}`} key={id}>
      <input type="checkbox" className="pick" checked={draft.done.includes(id)} onChange={(e) => tick(id, e.target.checked)} aria-label={`${title} done`} />
      <div className="cr-check-b">
        <b className="hi">{title}</b>
        <div className="cr-check-t">{body}</div>
      </div>
      {tools && <div className="cr-check-k">{tools}</div>}
    </div>
  );

  return (
    <div className="stack cr-new">
      <div className="onb-steps" aria-label="Steps">
        {["Check and pack", "Make the page on Nexus", "Show it in My mods"].map((s, i) => (
          <span key={s} className={`onb-dot${step === i + 1 ? " on" : step > i + 1 ? " done" : ""}`}>{i + 1} · {s}</span>
        ))}
      </div>

      <div className="grid2" style={{ alignItems: "start" }}>
        <div className="stack">
          <CheckModule pack={pack} compact />
          <PackModule pack={pack} />
        </div>

        <Module icon="web" title="Make the page on Nexus" count={`${draft.done.length} of ${ITEMS.length} done`}>
          <p>Nexus makes new mod pages on its website only. Open its form here, in the Nexus window, and tick each part off as you fill it in.</p>
          <div className="actions" style={{ marginTop: ".6em" }}>
            <Key variant="primary" icon="web" onClick={() => openNexus(mine?.links.addMod ?? "https://www.nexusmods.com/cyberpunk2077/mods/add")}>Open the add a mod page</Key>
            {draft.done.length > 0 && <Key size="sm" variant="ghost" onClick={() => save(blank())}>Start over</Key>}
          </div>
          <div className="cr-checks">
            {row("name", "Name", pack.name ? <span className="mono">{pack.name}</span> : <span className="dim">Set it in Pack it.</span>,
              pack.name && <Key size="sm" variant="ghost" icon="file" onClick={() => copy(pack.name)} aria-label="Copy the name" title="Copy" />)}
            {row("summary", "Summary", <>
              <textarea rows={3} value={draft.summary} onChange={(e) => save({ ...draft, summary: e.target.value })} aria-label="Summary" placeholder="One or two plain sentences: what it does and what it needs. It shows on cards." />
              <span className="dim cr-count">{draft.summary.length} characters</span>
            </>, draft.summary && <Key size="sm" variant="ghost" icon="file" onClick={() => copy(draft.summary)} aria-label="Copy the summary" title="Copy" />)}
            {row("category", "Category", (
              <select value={draft.category} onChange={(e) => save({ ...draft, category: e.target.value })} aria-label="Category">
                <option value="">Pick the closest one</option>
                {cats.map((c) => <option key={c} value={c}>{c}</option>)}
              </select>
            ), draft.category && <Key size="sm" variant="ghost" icon="file" onClick={() => copy(draft.category)} aria-label="Copy the category" title="Copy" />)}
            {row("file", "The file", zip ? <>Upload <span className="mono hi">{fileName(zip)}</span> as the Main file, version <span className="mono">{pack.version}</span>. Turn on mod manager downloads.</> : <span className="dim">Pack your folder first (left).</span>,
              zip && <Key size="sm" variant="ghost" icon="folder" onClick={() => copy(zip, "Path copied. Paste it in the file picker.")} aria-label="Copy the file's path" title="Copy the path" />)}
            {row("requirements", "Requirements", needs.length ? <>
              Add each one, so managers install them too:
              <div className="m-tags" style={{ marginTop: ".3em" }}>{needs.map((n) => (
                <button key={n.key} type="button" className="cr-tagk" onClick={() => copy(n.url, `${n.name}'s link copied.`)} title={`Copy ${n.url}`}><Tag kind={n.key}>{n.name}</Tag></button>
              ))}</div>
            </> : <span className="dim">{pack.check ? "None found in the files. Add any mod yours needs." : "Check your mod to see what it needs."}</span>)}
            {row("picture", "A picture", <span>At least one image. The first is the card picture in every browser, this one too.</span>)}
            {row("publish", "Publish", <span>Publish the page, or keep it hidden while Nexus scans the file. Apps see it once it is published.</span>)}
          </div>
        </Module>
      </div>

      <Module icon="user" title="Show it in My mods" count={fresh.length ? "found" : undefined}>
        {!mine ? <div className="actions">
          <span style={{ flex: 1, minWidth: "14em" }}>Show your mods first (sign in with your key, or type your user name), then come back here.</span>
          <Key icon="user" onClick={onOpenMine}>Go to My mods</Key>
        </div> : <>
          <p>A published mod shows in My mods by itself. Look for it now, or paste the link to its page.</p>
          <div className="actions" style={{ marginTop: ".6em" }}>
            <Key icon="refresh" disabled={looking} onClick={async () => { setLooking(true); try { await onRefresh(); } finally { setLooking(false); } }}>{looking ? "Looking" : "Look for it"}</Key>
          </div>
          {newest.length > 0 && <>
            <div className="sub-h">Your newest mods</div>
            <div className="rows">
              {newest.map((m) => (
                <button key={m.modId} type="button" className="row" onClick={onOpenMine}>
                  <span className="m-main"><span className="m-t">{m.name}</span><span className="m-meta"><span>Created {shortDate(m.createdAt)}</span><span>{m.status.replace(/_/g, " ")}</span></span></span>
                  <span className="m-side">{fresh.includes(m) ? <Pill tone="ok">New</Pill> : <Icon name="chevron" size={9} />}</span>
                </button>
              ))}
            </div>
          </>}
          <form className="cr-who" style={{ marginTop: ".7em" }} onSubmit={(e) => { e.preventDefault(); void addLink(); }}>
            <label className="fbox"><Icon name="link" /><input className="finput" value={link} onChange={(e) => setLink(e.target.value)} placeholder="Paste the new page's link or number" aria-label="New mod link or number" /></label>
            <Key type="submit" icon="plus" disabled={!link.trim()}>Add to My mods</Key>
          </form>
        </>}
      </Module>
    </div>
  );
}

const ITEMS = ["name", "summary", "category", "file", "requirements", "picture", "publish"];
