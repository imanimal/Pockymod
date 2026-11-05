// Health: one module per area, its band red when something is broken, and one orange key for the fix to do first.

import { useEffect, useState } from "react";
import { Deck } from "../components/Deck";
import { Icon } from "../components/Icon";
import { Key, Lcd, Module, Pill } from "../components/pocket";
import { api, pickFolder } from "../lib/api";
import { useAction, useApp } from "../lib/store";
import type { Fix, HealthReport, Status } from "../lib/types";

function pill(s: Status) {
  return s === "bad" ? <Pill tone="over">Broken</Pill> : s === "warn" ? <Pill tone="soon">Check</Pill> : <Pill tone="ok">OK</Pill>;
}

export function Health() {
  const { go, toast, refresh, libraryRev } = useApp();
  const [h, setH] = useState<HealthReport | null>(null);
  const { busy, run } = useAction();
  const reload = () => api.health().then(setH).catch(() => {});
  useEffect(() => {
    reload();
  }, [libraryRev]);

  const apply = (fix: Fix) =>
    run("fix", async () => {
      const a = fix.action;
      switch (a.type) {
        case "setGameFolder": {
          const p = await pickFolder("Pick the Cyberpunk 2077 folder");
          if (p) await api.setGameRoot(p);
          break;
        }
        case "installFramework":
        case "updateFramework":
          await api.queueMod(a.modId);
          toast(`${fix.label.replace(/^(Install|Update) /, "")} queued.`, { action: { label: "Open queue", run: () => go("downloads") } });
          break;
        case "runRedmodDeploy": {
          const out = await api.redmodDeploy();
          toast(out.ok ? "REDmods deployed." : `REDmod deploy failed: ${out.problems[0] ?? ""}`, out.ok ? undefined : { tone: "bad" });
          break;
        }
        case "installRedmodDlc":
          toast("REDmod is a free DLC. Steam: right-click the game, Properties, DLC, tick REDmod. GOG Galaxy: the game's page, DLC, install REDmod.");
          break;
        case "redeploy":
          await api.redeploy(a.key);
          toast("Files put back.");
          break;
        case "checkUpdates": {
          const n = await api.checkUpdates();
          toast(n ? `${n} update(s) found. Install them from Library.` : "No updates found.", n ? { action: { label: "Library", run: () => go("library") } } : undefined);
          break;
        }
      }
      await reload();
      refresh();
    });

  if (!h) return null;
  const primary = h.primaryFix;
  const primaryItem = h.modules.flatMap((m) => m.items).find((i) => i.fix && primary && JSON.stringify(i.fix.action) === JSON.stringify(primary.action));
  const isPrimary = (f: Fix | null) => !!f && !!primary && JSON.stringify(f.action) === JSON.stringify(primary.action);

  return (
    <div className="health">
      <Deck>
        <Key icon="refresh" onClick={reload}>
          Check again
        </Key>
        <Key icon="settings" onClick={() => go("settings")}>
          Game folder
        </Key>
      </Deck>
      <Module title="Summary" icon="health" auto band={h.worst === "bad" ? "bad" : undefined} className="h-summary">
        <div className="hero">
          <Lcd label="Problems" value={h.problems} tone={h.worst === "bad" ? "bad" : h.worst === "warn" ? "hot" : null} sub={h.worst === "ok" ? "Ready to play" : h.worst === "bad" ? "Fix before playing" : "Worth a look"} />
          <div className="hero-main">
            <h2 className="d-title">{h.worst === "ok" ? "Everything looks fine." : primaryItem ? primaryItem.title : "Some things need a look."}</h2>
            {primaryItem?.detail ? <p className="summary">{primaryItem.detail}</p> : null}
            {primary ? (
              <div className="actions">
                <Key variant="primary" size="lg" icon="check" disabled={busy === "fix"} onClick={() => apply(primary)}>
                  {primary.label}
                </Key>
              </div>
            ) : null}
          </div>
        </div>
      </Module>
      <div className="h-grid">
        {h.modules
          .filter((m) => m.items.length > 0)
          .map((m) => (
            <Module key={m.id} title={m.title} icon={m.status === "ok" ? "check" : "warn"} count={m.items.filter((i) => i.status !== "ok").length || null} band={m.status === "bad" ? "bad" : undefined} auto>
              <ul className="hitems">
                {m.items.map((it, i) => (
                  <li key={i} className={`hitem ${it.status}`}>
                    <Icon name={it.status === "ok" ? "check" : "warn"} />
                    <div className="m-main">
                      <span className="m-t wrap">{it.title}</span>
                      {it.detail ? <span className="m-meta wrap">{it.detail}</span> : null}
                    </div>
                    <div className="m-due">
                      {pill(it.status)}
                      {it.fix && !isPrimary(it.fix) ? (
                        <Key size="sm" disabled={busy === "fix"} onClick={() => apply(it.fix!)}>
                          {it.fix.label}
                        </Key>
                      ) : null}
                    </div>
                  </li>
                ))}
              </ul>
            </Module>
          ))}
      </div>
    </div>
  );
}
