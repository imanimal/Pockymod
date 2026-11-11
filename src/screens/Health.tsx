// Health: modules by area; a band turns red when something in it is broken. The most serious fixable issue gets
// the screen's one orange key; other fixes are dark keys.
import { plural } from "../lib/text";
import { useCallback, useEffect, useRef, useState } from "react";
import { Empty, Key, Lcd, Module, Pill, Tag } from "../ui/pocket";
import type { IconName } from "../ui/icons";
import { archives, errorText, game, library, nexusPages, queueMod, resetScripts, type Fix, type HealthReport, type Issue } from "../lib/api";
import { toast } from "../ui/toast";
import { LogsModule } from "./LogsModule";
import * as ops from "../lib/ops";

const AREAS: Array<{ id: Issue["area"]; title: string; icon: IconName }> = [
  { id: "game", title: "Game", icon: "play" },
  { id: "frameworks", title: "Frameworks", icon: "chip" },
  { id: "logs", title: "Logs", icon: "file" },
  { id: "deploy", title: "Files in the game", icon: "folder" },
  { id: "redmod", title: "REDmod", icon: "loadorder" },
  { id: "conflicts", title: "Overrides", icon: "warn" },
];

export function Health({ onNavigate, onIssues }: { onNavigate: (v: "library" | "settings" | "loadorder" | "downloads") => void; onIssues?: (n: number) => void }) {
  const [r, setR] = useState<HealthReport | null>(null);
  // the key at work: "check" for Check again, "all" or "frameworks" for the Fix all keys, an issue's id for its fix
  const [working, setWorking] = useState<string | null>(null);
  const busy = working !== null;
  const [err, setErr] = useState<string | null>(null);
  const [stamp, setStamp] = useState(0);
  // only the newest check's answer counts; a failed check keeps the last report on screen
  const seq = useRef(0);
  const have = useRef(false);
  const load = useCallback((refresh = false) => {
    const n = ++seq.current;
    setWorking("check");
    setStamp((k) => k + 1);
    game.health(refresh)
      .then((x) => { if (n !== seq.current) return; have.current = true; setR(x); setErr(null); onIssues?.(x.problems); })
      .catch((e) => { if (n !== seq.current) return; if (have.current) toast(errorText(e), true); else setErr(errorText(e)); })
      .finally(() => { if (n === seq.current) setWorking(null); });
  }, [onIssues]);
  useEffect(() => { load(); }, [load]);

  /** Run one fix; with `quiet` it neither navigates nor reloads (Fix all does that once at the end). A fix that
   *  doesn't work out throws, with the reason as its message. */
  const run = async (f: Fix, quiet = false): Promise<void> => {
    switch (f.kind) {
      case "installMod": case "updateMod": await queueMod(f.modId); if (!quiet) { toast(`${f.name} queued`); onNavigate("downloads"); } return;
      case "runRedmodDeploy": {
        const res = await game.redmodDeploy();
        if (!res.ok) throw new Error(`REDmod deploy hit ${plural(res.problems.length, "problem")}${res.problems[0] ? `: ${res.problems[0]}` : ""}`);
        if (!quiet) toast(`REDmod deploy done (${plural(res.mods.length, "mod")})`);
        return;
      }
      case "redeploy": {
        const d = await library.deploy(f.installId, { allowCopy: false, takeConflicts: false });
        if (d.outcome === "conflicts") throw new Error("Its files clash with other mods. Decide in Library");
        if (d.outcome === "needsCopy") throw new Error("It needs copying instead of linking. Decide in Library");
        if (!quiet) toast("Deployed again");
        return;
      }
      case "resetScripts": { const n = await resetScripts(); if (!quiet) toast(n ? "Game scripts reset" : "Nothing to reset"); return; }
      case "turnOff": {
        const row = (await library.list()).find((x) => x.id === f.installId);
        if (row) await ops.setEnabled([row], false);
        else toast(`${f.name} is no longer installed`);
        return;
      }
      case "disableModlist": { const done = await archives.disableModlist(); if (!quiet) toast(done ? "modlist.txt renamed to modlist.txt.off" : "No modlist.txt"); return; }
      case "openPage": await nexusPages.mod(f.modId); return;
      case "openLibrary": case "openSettings": return;
    }
  };
  /** Every fix the app can do by itself, in order: downloads first, files next, the REDmod deploy last (and only
   *  when nothing was queued, since Play deploys again once the queue has installed). Turning a mod off and the
   *  fixes that only open a screen are left to you. */
  const fixAll = async (list: Fix[], key: string) => {
    setWorking(key);
    const order: Fix["kind"][] = ["installMod", "updateMod", "redeploy", "resetScripts", "disableModlist", "runRedmodDeploy"];
    const todo = [...list].sort((a, b) => order.indexOf(a.kind) - order.indexOf(b.kind));
    let ok = 0, queued = 0;
    const failed: Array<{ fix: Fix; why: string }> = [];
    for (const f of todo) {
      if (f.kind === "runRedmodDeploy" && queued) continue;
      try {
        await run(f, true);
        if (f.kind === "installMod" || f.kind === "updateMod") queued++; else ok++;
      } catch (e) { failed.push({ fix: f, why: errorText(e) }); }
    }
    const done = [ok && `${plural(ok, "fix", "fixes")} done`, queued && `${plural(queued, "download")} queued`].filter(Boolean).join(", ");
    if (!failed.length) toast(done || "Nothing to fix");
    else {
      // one reason for all of them is said once; otherwise the first, named
      const why = failed.every((x) => x.why === failed[0].why) ? failed[0].why : `${fixLabel(failed[0].fix)}: ${failed[0].why}`;
      toast(`${done ? `${done}. ${failed.length} failed` : `Couldn't fix ${failed.length === 1 ? "it" : `any of the ${failed.length}`}`}. ${why}`, true);
    }
    load();
    if (queued) onNavigate("downloads");
  };

  const fix = async (i: Issue & { fix: Fix }) => {
    const f = i.fix;
    if (f.kind === "openLibrary") return onNavigate("library");
    if (f.kind === "openSettings") return onNavigate("settings");
    if (f.kind === "openPage") return run(f).catch((e) => toast(errorText(e), true));
    setWorking(i.id);
    try { await run(f); if (f.kind !== "installMod" && f.kind !== "updateMod") load(); else setWorking(null); }
    catch (e) { toast(errorText(e), true); setWorking(null); }
  };

  if (err) return (
    <Module icon="health" title="Health" band="bad"
      tools={<Key size="sm" icon="refresh" busy={working === "check"} onClick={() => load(true)}>Check again</Key>}>
      <div className="note bad">{err}</div>
    </Module>
  );
  if (!r) return <Module icon="health" title="Health" count="checking"><Empty icon="clock" title="Checking the game" /></Module>;
  const auto = (i: Issue): i is Issue & { fix: Fix } => !!i.fix && AUTO.includes(i.fix.kind) && i.area !== "logs";
  const fixable = unique(r.issues.filter((i) => auto(i) && i.severity !== "info").map((i) => i.fix!));
  const fwFixes = unique(r.issues.filter((i) => i.area === "frameworks" && (i.fix?.kind === "updateMod" || i.fix?.kind === "installMod")).map((i) => i.fix!));
  // with two or more fixes the orange key is Fix all; with one it is that fix
  const first = fixable.length > 1 ? undefined : r.issues.find((i) => i.fix && i.severity !== "info" && i.area !== "logs");
  const g = r.game;

  return (
    <div className="cols">
      <div className="lcds">
        <Lcd label="Problems" value={String(r.problems).padStart(2, "0")} tone={r.problems ? "bad" : "ok"} sub={r.problems ? "Fix before you play" : "Nothing broken"} />
        <Lcd label="Warnings" value={String(r.warnings).padStart(2, "0")} tone={r.warnings ? "hot" : "ok"} sub={r.warnings ? "Worth a look" : "All clear"} />
        <Lcd label="Game patch" value={r.patch ?? "?"} sub={r.patchSource === "detected" ? "From the game" : r.patchSource === "settings" ? "Set in Settings" : "Guessed, set it in Settings"} tone={r.patchSource === "assumed" ? "hot" : "ok"} />
      </div>
      <div className="actions span">
        {fixable.length > 1 && <Key variant="primary" icon="check" onClick={() => fixAll(fixable, "all")} disabled={busy} busy={working === "all"}
          title="Run every automatic fix">Fix all {fixable.length}</Key>}
        <Key size="sm" icon="refresh" onClick={() => load(true)} disabled={busy} busy={working === "check"}>Check again</Key>
        {g?.version && <span className="dim mono" style={{ fontSize: ".8em" }}>Cyberpunk2077.exe {g.version.productString ?? g.version.product}</span>}
      </div>

      {/* an area with a problem goes first, so the Problems count always has something red under it */}
      {[...AREAS].sort((a, b) => Number(hasProblem(r, b.id)) - Number(hasProblem(r, a.id))).map((a) => {
        const list = r.issues.filter((i) => i.area === a.id);
        if (a.id === "conflicts" && list.length === 0) return null;
        if (a.id === "logs") return g ? <LogsModule key="logs" stamp={stamp} onChanged={() => load()} /> : null;
        const bad = list.some((i) => i.severity === "problem");
        return (
          <Module key={a.id} icon={a.icon} title={a.title} band={bad ? "bad" : undefined} count={list.length ? `${list.length} ${list.length === 1 ? "item" : "items"}` : "ok"}
            tools={a.id === "frameworks" && fwFixes.length > 1 ? <Key size="sm" variant="dark" icon="downloads" disabled={busy} busy={working === "frameworks"} onClick={() => fixAll(fwFixes, "frameworks")}
              title="Queue missing and outdated frameworks">{fwFixes.every((f) => f.kind === "updateMod") ? `Update all ${fwFixes.length}` : `Install and update all ${fwFixes.length}`}</Key> : undefined}>
            {a.id === "frameworks" && <FrameworkTable r={r} />}
            {list.length === 0 && a.id !== "frameworks" && <p className="dim">{a.id === "redmod" ? (g?.redmodAvailable ? "Up to date" : "No REDmods") : "All good"}</p>}
            <div className="rows" style={{ marginTop: a.id === "frameworks" && list.length ? ".6em" : 0 }}>
              {list.map((i) => (
                <div key={i.id} className="row">
                  <span className="m-main">
                    <span className="m-t" style={{ whiteSpace: "normal" }}>{i.title}</span>
                    <span style={{ fontSize: ".86em", color: "var(--text-2)" }}>{i.detail}</span>
                  </span>
                  <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".5em" }}>
                    <Pill tone={i.severity === "problem" ? "over" : i.severity === "warning" ? "soon" : undefined}>{i.severity === "problem" ? "Problem" : i.severity === "warning" ? "Warning" : "Note"}</Pill>
                    {i.fix && <Key size="sm" variant={i === first ? "primary" : "dark"} onClick={() => fix(i as Issue & { fix: Fix })} disabled={busy} busy={working === i.id}
                      title={i.fix.kind === "openPage" ? `Open ${i.fix.name} on Nexus to check for an update` : undefined}>{fixLabel(i.fix)}</Key>}
                  </span>
                </div>
              ))}
            </div>
          </Module>
        );
      })}
    </div>
  );
}

const hasProblem = (r: HealthReport, area: Issue["area"]) => r.issues.some((i) => i.area === area && i.severity === "problem");

/** The fixes Fix all runs by itself. */
const AUTO: Fix["kind"][] = ["installMod", "updateMod", "runRedmodDeploy", "redeploy", "resetScripts", "disableModlist"];
/** One of each fix (two issues can ask for the same download or the same deploy). */
function unique(list: Fix[]): Fix[] {
  const seen = new Set<string>();
  return list.filter((f) => { const k = JSON.stringify(f.kind === "installMod" || f.kind === "updateMod" ? { k: "mod", id: f.modId } : f); if (seen.has(k)) return false; seen.add(k); return true; });
}

function fixLabel(f: Fix): string {
  switch (f.kind) {
    case "installMod": return `Install ${f.name}`;
    case "updateMod": return `Update ${f.name}`;
    case "runRedmodDeploy": return "Run REDmod deploy";
    case "redeploy": return "Deploy again";
    case "openLibrary": return "Open Library";
    case "openSettings": return "Open Settings";
    case "resetScripts": return "Reset scripts";
    case "turnOff": return "Turn this mod off";
    case "disableModlist": return "Move it aside";
    case "openPage": return "Nexus page";
  }
}

function FrameworkTable({ r }: { r: HealthReport }) {
  return (
    <div className="rows">
      {r.frameworks.map((f) => (
        <div key={f.key} className={`row${f.installed ? "" : " off"}`} style={{ cursor: "default" }}>
          <span className="m-main">
            <span className="m-t"><Tag kind={f.key}>{f.name}</Tag></span>
            <span className="m-meta">
              <span>{f.installed ? `Installed ${f.version ?? "(version unknown)"}` : "Not installed"}</span>
              {f.latest && <span>Nexus {f.latest}</span>}
              {f.neededBy.length > 0 && <span>needed by {f.neededBy.length} {f.neededBy.length === 1 ? "mod" : "mods"}</span>}
            </span>
          </span>
          <span className="m-side">
            {f.installed ? (f.olderThanGame ? <Pill tone="over">Older than the patch</Pill> : f.updateAvailable ? <Pill tone="soon">Update</Pill> : <Pill tone="ok">OK</Pill>)
              : f.neededBy.length ? <Pill tone="over">Missing</Pill> : <Pill>Not needed</Pill>}
          </span>
        </div>
      ))}
    </div>
  );
}
