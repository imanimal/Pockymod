// Health: modules by area; a band turns red when something in it is broken. The most serious fixable issue gets
// the screen's one orange key; other fixes are dark keys.
import { plural } from "../lib/text";
import { useCallback, useEffect, useState } from "react";
import { Empty, Key, Lcd, Module, Pill, Tag } from "../ui/pocket";
import type { IconName } from "../ui/icons";
import { archives, errorText, game, library, queueMod, resetScripts, type Fix, type HealthReport, type Issue } from "../lib/api";
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
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [stamp, setStamp] = useState(0);
  const load = useCallback((refresh = false) => {
    setBusy(true);
    setStamp((n) => n + 1);
    game.health(refresh).then((x) => { setR(x); setErr(null); onIssues?.(x.problems); }).catch((e) => setErr(errorText(e))).finally(() => setBusy(false));
  }, [onIssues]);
  useEffect(() => { load(); }, [load]);

  const fix = async (f: Fix) => {
    try {
      switch (f.kind) {
        case "installMod": case "updateMod": await queueMod(f.modId); toast(`${f.name} is queued.`); onNavigate("downloads"); return;
        case "runRedmodDeploy": {
          setBusy(true);
          const res = await game.redmodDeploy();
          toast(res.ok ? `REDmod deploy done (${plural(res.mods.length, "mod")}).` : `REDmod deploy reported ${plural(res.problems.length, "problem")}.`, !res.ok);
          break;
        }
        case "redeploy": await library.deploy(f.installId, { allowCopy: false, takeConflicts: false }); toast("Deployed again."); break;
        case "openLibrary": onNavigate("library"); return;
        case "openSettings": onNavigate("settings"); return;
        case "resetScripts": { const n = await resetScripts(); toast(n ? "The game's own scripts are back." : "Nothing needed resetting."); break; }
        case "turnOff": {
          const row = (await library.list()).find((x) => x.id === f.installId);
          if (row) await ops.setEnabled([row], false);
          break;
        }
        case "disableModlist": { const done = await archives.disableModlist(); toast(done ? "modlist.txt is set aside (modlist.txt.off). File names decide the order again." : "modlist.txt is already gone."); break; }
      }
      load();
    } catch (e) { toast(errorText(e), true); setBusy(false); }
  };

  if (err) return <Module icon="health" title="Health" band="bad"><div className="note bad">{err}</div></Module>;
  if (!r) return <Module icon="health" title="Health" count="checking"><Empty icon="clock" title="Checking the game" /></Module>;
  const first = r.issues.find((i) => i.fix && i.severity !== "info" && i.area !== "logs");
  const g = r.game;

  return (
    <div className="cols">
      <div className="lcds">
        <Lcd label="Problems" value={String(r.problems).padStart(2, "0")} tone={r.problems ? "bad" : "ok"} sub={r.problems ? "Fix before you play" : "Nothing broken"} />
        <Lcd label="Warnings" value={String(r.warnings).padStart(2, "0")} tone={r.warnings ? "hot" : "ok"} sub={r.warnings ? "Worth a look" : "All clear"} />
        <Lcd label="Game patch" value={r.patch ?? "?"} sub={r.patchSource === "detected" ? "Read from the game" : r.patchSource === "settings" ? "Set in Settings" : "Assumed: set it in Settings"} tone={r.patchSource === "assumed" ? "hot" : "ok"} />
      </div>
      <div className="actions span">
        <Key size="sm" icon="refresh" onClick={() => load(true)} disabled={busy}>Check again</Key>
        {g?.version && <span className="dim mono" style={{ fontSize: ".8em" }}>Cyberpunk2077.exe {g.version.productString ?? g.version.product}</span>}
      </div>

      {AREAS.map((a) => {
        const list = r.issues.filter((i) => i.area === a.id);
        if (a.id === "conflicts" && list.length === 0) return null;
        if (a.id === "logs") return g ? <LogsModule key="logs" stamp={stamp} onChanged={() => load()} /> : null;
        const bad = list.some((i) => i.severity === "problem");
        return (
          <Module key={a.id} icon={a.icon} title={a.title} band={bad ? "bad" : undefined} count={list.length ? `${list.length} ${list.length === 1 ? "item" : "items"}` : "ok"}>
            {a.id === "frameworks" && <FrameworkTable r={r} />}
            {list.length === 0 && a.id !== "frameworks" && <p className="dim">{a.id === "redmod" ? (g?.redmodAvailable ? "REDmod is installed and up to date." : "No REDmods installed.") : "All good."}</p>}
            <div className="rows" style={{ marginTop: a.id === "frameworks" && list.length ? ".6em" : 0 }}>
              {list.map((i) => (
                <div key={i.id} className="row">
                  <span className="m-main">
                    <span className="m-t" style={{ whiteSpace: "normal" }}>{i.title}</span>
                    <span style={{ fontSize: ".86em", color: "var(--text-2)" }}>{i.detail}</span>
                  </span>
                  <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".5em" }}>
                    <Pill tone={i.severity === "problem" ? "over" : i.severity === "warning" ? "soon" : undefined}>{i.severity === "problem" ? "Problem" : i.severity === "warning" ? "Warning" : "Note"}</Pill>
                    {i.fix && <Key size="sm" variant={i === first ? "primary" : "dark"} onClick={() => fix(i.fix!)} disabled={busy}>{fixLabel(i.fix)}</Key>}
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
