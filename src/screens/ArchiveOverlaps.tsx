// Load order's archive modules: mods whose .archive files change the same game files (read from each archive's
// file table), who wins and how to change it; and the user's standing choices (who wins a file clash, archives
// renamed to load first), each with a way to drop it.
import { useCallback, useEffect, useState } from "react";
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
    let off: (() => void) | undefined;
    library.onChanged(load).then((u) => { off = u; });
    return () => off?.();
  }, [load]);
}

export function OverlapsModule() {
  const [r, setR] = useState<OverlapReport | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [all, setAll] = useState(false);
  const load = useCallback(() => { archives.overlaps().then((x) => { setR(x); setErr(null); }).catch((e) => setErr(errorText(e))); }, []);
  useLibraryReload(load);

  const makeWin = async (o: Overlap) => {
    if (!o.loadFirst || o.loser.installId == null) return;
    setBusy(true);
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
    } catch (e) { toast(errorText(e), true); } finally { setBusy(false); load(); }
  };

  if (err) return <Module icon="warn" title="Overlapping archives"><div className="note bad">{err}</div></Module>;
  if (!r) return <Module icon="warn" title="Overlapping archives" count="reading"><p className="dim">Reading archives</p></Module>;
  const shown = all ? r.overlaps : r.overlaps.slice(0, 8);
  return (
    <Module icon="warn" title="Overlapping archives" count={r.overlaps.length ? plural(r.overlaps.length, "pair") : "none"} band={r.modlist ? "hot" : undefined}
      tools={<Key size="sm" variant="ghost" icon="refresh" onClick={load} aria-label="Check again" title="Check again" />}>
      <p className="dim" style={{ fontSize: ".86em", marginBottom: ".5em" }}>Archives load in name order. The first one wins</p>
      {r.modlist && (
        <div className="note warn" style={{ marginBottom: ".6em" }}>
          archive/pc/mod/modlist.txt sets the order ({plural(r.modlist.length, "archive")} listed)
          <div className="actions" style={{ marginTop: ".4em" }}>
            <Key size="sm" disabled={busy} onClick={async () => { setBusy(true); try { await archives.disableModlist(); toast("modlist.txt renamed to modlist.txt.off"); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); load(); } }}>Move it aside</Key>
          </div>
        </div>
      )}
      {r.overlaps.length === 0 && <Empty icon="check" title="No clashes">{r.archives ? `${plural(r.archives, "archive")} read` : ""}</Empty>}
      <div className="rows">
        {shown.map((o) => (
          <div key={`${o.winner.name}>${o.loser.name}`} className="row ovrow" style={{ cursor: "default" }}>
            <span className="m-main">
              <span className="m-t" style={{ whiteSpace: "normal" }}><b className="okc">{o.winner.name}</b> <span className="dim">wins over</span> {o.loser.name}</span>
              <span className="m-meta">
                <span>{plural(o.files, "game file")} both change</span>
                {o.loser.redmod && <span>REDmod</span>}
              </span>
              <span className="logline" title={[...o.winner.archives, ...o.loser.archives].join("\n")}>{o.winner.archives.map(file).join(", ")} loads before {o.loser.archives.map(file).join(", ")}</span>
              {!o.loadFirst && <span className="ovhow">{o.how}</span>}
            </span>
            <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".3em" }}>
              {o.loadFirst && <Key size="sm" disabled={busy} onClick={() => makeWin(o)} title={`Renames ${o.loadFirst.map((x) => `${file(x.target)} to ${x.name}`).join(", ")}`}>Make {o.loser.name} win</Key>}
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
  const [wins, setWins] = useState<WinRule[]>([]);
  const [renames, setRenames] = useState<RenameRule[]>([]);
  const [busy, setBusy] = useState(false);
  const load = useCallback(() => { rules.list().then((x) => { setWins(x.wins); setRenames(x.renames); }).catch(() => {}); }, []);
  useLibraryReload(load);
  const run = async (f: () => Promise<unknown>, done: string) => { setBusy(true); try { await f(); toast(done); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); load(); } };
  const n = wins.length + renames.length;
  return (
    <Module icon="key" title="Your choices" count={n ? plural(n, "rule") : "none"}>
      {n === 0 ? (
        <p className="dim" style={{ fontSize: ".88em" }}>None yet</p>
      ) : (
        <div className="rows">
          {wins.map((w) => (
            <div key={`w${w.id}`} className="row" style={{ cursor: "default" }}>
              <span className="m-main">
                <span className="m-t" style={{ whiteSpace: "normal" }}><b>{w.winner}</b> <span className="dim">wins over</span> {w.loser}</span>
                <span className="m-meta"><span>{plural(w.files, "file")} both want</span><span>since {date(w.createdAt)}</span></span>
              </span>
              <span className="m-side"><Key size="sm" variant="ghost" icon="x" disabled={busy} title="You'll be asked again next time"
                onClick={() => run(() => rules.forget(w.id), `Forgot ${w.winner} over ${w.loser}`)}>Forget</Key></span>
            </div>
          ))}
          {renames.map((x) => (
            <div key={`r${x.modId}${x.target}`} className="row" style={{ cursor: "default" }}>
              <span className="m-main">
                <span className="m-t">{x.name}</span>
                <span className="m-meta"><span className="mono">{file(x.target)}</span><span className="mono">loads as {x.renamed}</span><span>since {date(x.createdAt)}</span></span>
              </span>
              <span className="m-side"><Key size="sm" variant="ghost" icon="undo" disabled={busy} title="Restore the original name"
                onClick={() => run(() => archives.undoRename(x.modId, x.target), `${file(x.target)} restored`)}>Undo</Key></span>
            </div>
          ))}
        </div>
      )}
    </Module>
  );
}
