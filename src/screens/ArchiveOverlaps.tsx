// Load order's archive modules: mods whose .archive files change the same game files (read from each archive's
// file table), who wins and how to change it; and the user's standing choices (who wins a file clash, archives
// renamed to load first), each with a way to drop it.
import { useCallback, useEffect, useRef, useState } from "react";
import { Empty, Key, Module } from "../ui/pocket";
import { archives, errorText, library, rules, type Overlap, type OverlapReport, type RenameRule, type WinRule } from "../lib/api";
import { record } from "../lib/history";
import { toast } from "../ui/toast";

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const file = (p: string) => p.split("/").pop() ?? p;
const date = (unix: number) => new Date(unix * 1000).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" });

function useLibraryReload(load: () => void) {
  useEffect(() => {
    load();
    let off: (() => void) | undefined, gone = false;
    library.onChanged(load).then((u) => { if (gone) u(); else off = u; });
    return () => { gone = true; off?.(); };
  }, [load]);
}

export function OverlapsModule() {
  const [r, setR] = useState<OverlapReport | null>(null);
  const [err, setErr] = useState<string | null>(null);
  // the key at work: an overlap's key, "modlist", or "check" while the archives are read again
  const [busy, setBusy] = useState<string | null>(null);
  const [all, setAll] = useState(false);
  // only the newest read counts: every library change starts one
  const seq = useRef(0);
  const load = useCallback(() => {
    const n = ++seq.current;
    archives.overlaps().then((x) => { if (n === seq.current) { setR(x); setErr(null); } }).catch((e) => { if (n === seq.current) setErr(errorText(e)); })
      .finally(() => { if (n === seq.current) setBusy((b) => b === "check" ? null : b); });
  }, []);
  useLibraryReload(load);
  const again = () => { setBusy("check"); load(); };
  const keyOf = (o: Overlap) => `${o.winner.name}>${o.loser.name}`;

  const makeWin = async (o: Overlap) => {
    if (!o.loadFirst || o.loser.installId == null) return;
    setBusy(keyOf(o));
    try {
      const id = o.loser.installId;
      const renames = o.loadFirst;
      const out = await archives.loadFirst(id, renames);
      if (out.outcome !== "deployed") toast(`${o.loser.name} renamed but not deployed. ${out.outcome === "conflicts" ? "Its files clash, decide in Library" : out.reason}`, true);
      const modId = o.loser.modId;
      if (modId != null) {
        record({
          label: `Made ${o.loser.name} load before ${o.winner.name}`,
          undo: async () => { for (const x of renames) await archives.undoRename(modId, x.target); },
          redo: () => archives.loadFirst(id, renames),
        });
      } else toast(`${o.loser.name} now loads first`);
    } catch (e) { toast(errorText(e), true); } finally { setBusy(null); load(); }
  };
  const checkKey = <Key size="sm" variant="ghost" icon="refresh" disabled={busy !== null} busy={busy === "check"} onClick={again} aria-label="Check again" title="Read the archives again" />;

  if (err) return <Module icon="warn" title="Overlapping archives" tools={checkKey}><div className="note bad">{err}</div></Module>;
  if (!r) return <Module icon="warn" title="Overlapping archives" count="reading"><p className="dim">Reading archives</p></Module>;
  const shown = all ? r.overlaps : r.overlaps.slice(0, 8);
  return (
    <Module icon="warn" title="Overlapping archives" count={r.overlaps.length ? plural(r.overlaps.length, "pair") : "none"} band={r.modlist ? "hot" : undefined}
      tools={checkKey}>
      <p className="dim" style={{ fontSize: ".86em", marginBottom: ".5em" }}>Archives load in name order. The first one wins</p>
      {r.modlist && (
        <div className="note warn" style={{ marginBottom: ".6em" }}>
          archive/pc/mod/modlist.txt sets the order ({plural(r.modlist.length, "archive")} listed)
          <div className="actions" style={{ marginTop: ".4em" }}>
            <Key size="sm" disabled={busy !== null} busy={busy === "modlist"} title="Renames it to modlist.txt.off, so archives load by name again"
              onClick={async () => { setBusy("modlist"); try { await archives.disableModlist(); toast("modlist.txt renamed to modlist.txt.off"); } catch (e) { toast(errorText(e), true); } finally { setBusy(null); load(); } }}>Move it aside</Key>
          </div>
        </div>
      )}
      {r.overlaps.length === 0 && <Empty icon="check" title="No clashes">{r.archives ? `${plural(r.archives, "archive")} read` : ""}</Empty>}
      <div className="rows">
        {shown.map((o) => (
          <div key={keyOf(o)} className="row ovrow" style={{ cursor: "default" }}>
            <span className="m-main">
              <span className="m-t" style={{ whiteSpace: "normal" }}><b className="okc">{o.winner.name}</b> <span className="dim">wins over</span> {o.loser.name}</span>
              <span className="m-meta">
                <span>{plural(o.files, "game file")} both change</span>
                {o.loser.redmod && <span>REDmod</span>}
              </span>
              <span className="logline" title={[...o.winner.archives, ...o.loser.archives].join("\n")}>{o.winner.archives.map(file).join(", ")} {o.winner.archives.length > 1 ? "load" : "loads"} before {o.loser.archives.map(file).join(", ")}</span>
              {!o.loadFirst && <span className="ovhow">{o.how}</span>}
            </span>
            <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".3em" }}>
              {o.loadFirst && <Key size="sm" disabled={busy !== null} busy={busy === keyOf(o)} onClick={() => makeWin(o)} title={`Renames ${o.loadFirst.map((x) => `${file(x.target)} to ${x.name}`).join(", ")}`}>Make {o.loser.name} win</Key>}
            </span>
          </div>
        ))}
      </div>
      {r.overlaps.length > shown.length && <button type="button" className="linkish fold-line" onClick={() => setAll(true)}>{plural(r.overlaps.length - shown.length, "more pair")}, show all</button>}
      <p className="dim" style={{ fontSize: ".78em", marginTop: ".5em" }}>
        {plural(r.archives, "archive")} read in {r.millis} ms{r.sameContent ? `, ${plural(r.sameContent, "identical file")}` : ""}
        {r.unreadable.length ? `. Couldn't read ${r.unreadable.map(([p]) => file(p)).join(", ")}` : ""}
      </p>
    </Module>
  );
}

export function RulesModule() {
  const [list, setList] = useState<{ wins: WinRule[]; renames: RenameRule[] } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  // the key at work: "w" or "r" and the rule it drops
  const [busy, setBusy] = useState<string | null>(null);
  const seq = useRef(0);
  const load = useCallback(() => {
    const n = ++seq.current;
    rules.list().then((x) => { if (n === seq.current) { setList(x); setErr(null); } }).catch((e) => { if (n === seq.current) setErr(errorText(e)); });
  }, []);
  useLibraryReload(load);
  const run = async (key: string, f: () => Promise<unknown>, done: string) => { setBusy(key); try { await f(); toast(done); } catch (e) { toast(errorText(e), true); } finally { setBusy(null); load(); } };
  const wins = list?.wins ?? [], renames = list?.renames ?? [];
  const n = wins.length + renames.length;
  return (
    <Module icon="key" title="Your choices" count={list ? (n ? plural(n, "rule") : "none") : undefined}>
      {err && !list ? (
        <div className="note bad">{err}</div>
      ) : !list ? (
        <span className="skel skel-line" style={{ width: "50%" }} aria-hidden="true" />
      ) : n === 0 ? (
        <p className="dim" style={{ fontSize: ".88em" }}>None yet</p>
      ) : (
        <div className="rows">
          {wins.map((w) => (
            <div key={`w${w.id}`} className="row" style={{ cursor: "default" }}>
              <span className="m-main">
                <span className="m-t" style={{ whiteSpace: "normal" }}><b>{w.winner}</b> <span className="dim">wins over</span> {w.loser}</span>
                <span className="m-meta"><span>{plural(w.files, "file")} both want</span><span>since {date(w.createdAt)}</span></span>
              </span>
              <span className="m-side"><Key size="sm" variant="ghost" icon="x" disabled={busy !== null} busy={busy === `w${w.id}`} title="You'll be asked again next time"
                onClick={() => run(`w${w.id}`, () => rules.forget(w.id), `Forgot ${w.winner} over ${w.loser}`)}>Forget</Key></span>
            </div>
          ))}
          {renames.map((x) => (
            <div key={`r${x.modId}${x.target}`} className="row" style={{ cursor: "default" }}>
              <span className="m-main">
                <span className="m-t">{x.name}</span>
                <span className="m-meta"><span className="mono">{file(x.target)}</span><span className="mono">loads as {x.renamed}</span><span>since {date(x.createdAt)}</span></span>
              </span>
              <span className="m-side"><Key size="sm" variant="ghost" icon="undo" disabled={busy !== null} busy={busy === `r${x.modId}${x.target}`} title="Restore the original name"
                onClick={() => run(`r${x.modId}${x.target}`, () => archives.undoRename(x.modId, x.target), `${file(x.target)} restored`)}>Undo</Key></span>
            </div>
          ))}
        </div>
      )}
    </Module>
  );
}
