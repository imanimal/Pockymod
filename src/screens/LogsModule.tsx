// Health's Logs module: the tools' newest logs (RED4ext, its plugins, ArchiveXL, TweakXL, Codeware, redscript,
// CET and its mods), the last start's errors grouped by the mod they point at, and a report to paste on a forum.
import { useCallback, useEffect, useState } from "react";
import { Key, Lcd, Module, Pill } from "../ui/pocket";
import { errorText, library, logs, type LogEntry, type LogReport, type ModErrors } from "../lib/api";
import { toast } from "../ui/toast";
import * as ops from "../lib/ops";

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

export function ago(unix: number | null): string {
  if (!unix) return "time unknown";
  const s = Math.max(0, Date.now() / 1000 - unix);
  if (s < 90) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  if (s < 86400) return `${Math.round(s / 3600)} h ago`;
  return `${Math.round(s / 86400)} days ago`;
}

function clock(unix: number | null): string {
  if (!unix) return "--:--";
  const d = new Date(unix * 1000);
  const today = new Date().toDateString() === d.toDateString();
  return today ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : d.toLocaleDateString([], { day: "numeric", month: "short" });
}

async function copyText(text: string) {
  try { await navigator.clipboard.writeText(text); return; } catch { /* fall back below */ }
  const t = document.createElement("textarea");
  t.value = text;
  document.body.appendChild(t);
  t.select();
  document.execCommand("copy");
  t.remove();
}

/** `stamp` changes when Health checks again, so the logs are read again too. */
export function LogsModule({ stamp, onChanged }: { stamp: number; onChanged: () => void }) {
  const [r, setR] = useState<LogReport | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [open, setOpen] = useState<Set<number>>(new Set());
  const [showOther, setShowOther] = useState(false);
  const load = useCallback(() => { logs.scan().then((x) => { setR(x); setErr(null); }).catch((e) => setErr(errorText(e))); }, []);
  useEffect(() => { load(); }, [load, stamp]);

  const openLog = (path: string) => logs.open(path).catch((e) => toast(errorText(e), true));
  const turnOff = async (m: ModErrors) => {
    setBusy(true);
    try {
      const row = (await library.list()).find((x) => x.id === m.installId);
      if (!row) throw new Error(`${m.name} is no longer installed`);
      await ops.setEnabled([row], false);
      load(); onChanged();
    } catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
  };
  // every mod with errors in the last start, turned off in one go (one undo puts them all back)
  const turnOffAll = async (list: ModErrors[]) => {
    setBusy(true);
    try {
      const rows = (await library.list()).filter((x) => list.some((m) => m.installId === x.id));
      await ops.setEnabled(rows, false);
      toast(`Turned off ${plural(rows.length, "mod")} with errors. Ctrl+Z turns them back on.`);
      load(); onChanged();
    } catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
  };
  const copyReport = async () => {
    try { await copyText(await logs.report()); toast("Report copied. Paste it in a forum post or on the mod's bugs page."); }
    catch (e) { toast(errorText(e), true); }
  };

  if (err) return <Module icon="file" title="Logs" className="span"><div className="note bad">{err}</div></Module>;
  if (!r) return <Module icon="file" title="Logs" count="reading" className="span"><p className="dim">Reading the logs…</p></Module>;
  const current = r.logs.filter((l) => l.current);
  const older = r.logs.length - current.length;

  return (
    <Module icon="file" title="Logs" className="span logs-mod" band={r.errors ? "bad" : undefined}
      count={r.logs.length ? `${plural(r.logs.length, "log")}${older ? `, ${older} older` : ""}` : "none yet"}
      tools={<>
        {r.mods.filter((m) => m.enabled && m.errors > 0).length > 1 && <Key size="sm" variant="dark" icon="power" disabled={busy}
          onClick={() => turnOffAll(r.mods.filter((m) => m.enabled && m.errors > 0))} title="Turn off every mod the logs name with an error; Ctrl+Z turns them back on">
          Turn off all {r.mods.filter((m) => m.enabled && m.errors > 0).length} with errors</Key>}
        <Key size="sm" icon="share" onClick={copyReport} disabled={!r.logs.length} title="A plain-text summary for a forum post: game version, frameworks, mods on, errors">Copy report</Key>
      </>}>
      {r.logs.length === 0 ? (
        <p className="dim">No logs yet. RED4ext, redscript, Cyber Engine Tweaks and their plugins write logs when the game starts; start it once, then check again.</p>
      ) : (
        <div className="logs-grid">
          <div className="stack" style={{ gap: ".6em", minWidth: 0 }}>
            <div className="lcds">
              <Lcd size="sm" label="Errors" value={String(r.errors).padStart(2, "0")} tone={r.errors ? "bad" : "ok"} sub="since the last start" />
              <Lcd size="sm" label="Warnings" value={String(r.warnings).padStart(2, "0")} tone={r.warnings ? "hot" : "ok"} sub={r.mods.length ? `${plural(r.mods.length, "mod")} named` : "no mod named"} />
              <Lcd size="sm" label="Last start" value={clock(r.lastStart)} sub={r.lastStart ? ago(r.lastStart) : "not known"} />
            </div>
            {r.scriptsFailed && <div className="note bad">redscript says the scripts did not compile on the last start, so script mods do nothing. Turning off the mod it names, then starting again, usually tells which one.</div>}
            {r.mods.length === 0 && !r.other.length && <p className="dim">No errors or warnings in the logs of the last start.</p>}
            {r.mods.length > 0 && <div className="sub-h">By mod</div>}
            <div className="rows">
              {r.mods.map((m) => {
                const first = m.entries[0];
                const isOpen = open.has(m.installId);
                const tools = [...new Set(m.entries.map((e) => e.tool))];
                return (
                  <div key={`${m.installId}-${m.name}`} className={`row logrow${m.enabled ? "" : " off"}`}>
                    <span className="m-main">
                      <span className="m-t">{m.name}</span>
                      <span className="m-meta">
                        {m.errors > 0 && <span className="badc">{plural(m.errors, "error")}</span>}
                        {m.warnings > 0 && <span>{plural(m.warnings, "warning")}</span>}
                        <span>{tools.join(", ")}</span>
                        {!m.enabled && <span>off now</span>}
                      </span>
                      {first && !isOpen && <span className="logline" title={first.text}>{first.text.split("\n")[0]}</span>}
                      {isOpen && <div className="log" style={{ margin: ".4em 0 .2em" }}>{m.entries.map(entryText).join("\n\n")}</div>}
                    </span>
                    <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".3em" }}>
                      {m.enabled && <Key size="sm" variant={m.errors ? "dark" : "ghost"} icon="power" disabled={busy} onClick={() => turnOff(m)}>Turn this mod off</Key>}
                      {first && <Key size="sm" variant="ghost" icon="file" onClick={() => openLog(first.log)} title={first.log}>Open log</Key>}
                      <Key size="sm" variant="ghost" icon="chevron" className={isOpen ? "rot-key" : ""} aria-label={isOpen ? "Hide lines" : "Show lines"}
                        onClick={() => setOpen((s) => { const n = new Set(s); n.has(m.installId) ? n.delete(m.installId) : n.add(m.installId); return n; })} />
                    </span>
                  </div>
                );
              })}
            </div>
            {r.other.length > 0 && (
              <>
                <div className="sub-h">Not tied to one mod</div>
                {showOther ? <div className="log">{r.other.map(entryText).join("\n\n")}</div>
                  : <button type="button" className="linkish fold-line" onClick={() => setShowOther(true)}>
                      {otherText(r.other)} {r.other.length === 1 ? "names" : "name"} no installed mod (the tool's own, or a mod Pocket Mods does not manage). Show {r.other.length === 1 ? "it" : "them"}.
                    </button>}
              </>
            )}
          </div>
          <div style={{ minWidth: 0 }}>
            <div className="sub-h">Log files</div>
            <div className="rows">
              {r.logs.map((l) => (
                <div key={l.path} className={`row${l.current ? "" : " off"}`} style={{ cursor: "default" }}>
                  <span className="m-main">
                    <span className="m-t">{l.tool}</span>
                    <span className="m-meta"><span className="mono" title={l.path}>{l.path.split("/").pop()}</span><span>{ago(l.written)}</span></span>
                  </span>
                  <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".35em" }}>
                    {!l.current ? <Pill>Older than the last start</Pill>
                      : l.errors ? <Pill tone="over">{plural(l.errors, "error")}</Pill>
                      : l.warnings ? <Pill tone="soon">{plural(l.warnings, "warning")}</Pill> : <Pill tone="ok">Clean</Pill>}
                    <Key size="sm" variant="ghost" icon="file" aria-label={`Open ${l.path}`} title="Open log" onClick={() => openLog(l.path)} />
                  </span>
                </div>
              ))}
            </div>
            <p className="dim" style={{ fontSize: ".8em", marginTop: ".5em" }}>Only logs written since the game last started count. An older log is from a run before that, or from a tool that did not load.</p>
          </div>
        </div>
      )}
    </Module>
  );
}

function otherText(list: LogEntry[]): string {
  const e = list.filter((x) => x.level === "error").length;
  const w = list.length - e;
  return [e && plural(e, "error"), w && plural(w, "warning")].filter(Boolean).join(" and ");
}

function entryText(e: LogEntry): string {
  return `${e.tool} ${e.level === "error" ? "error" : "warning"}${e.time ? ` (${e.time})` : ""}\n${e.text}`;
}
