// Load order: REDmods in the order the game loads them, and the deploy step. "Deploy REDmods" is orange.

import { useEffect, useState } from "react";
import { Deck } from "../components/Deck";
import { Empty, Key, Lcd, Module, Pill } from "../components/pocket";
import { api } from "../lib/api";
import { date } from "../lib/format";
import { useAction, useApp } from "../lib/store";
import type { LoadOrderView } from "../lib/types";

export function LoadOrder() {
  const { toast, libraryRev, refresh } = useApp();
  const [v, setV] = useState<LoadOrderView | null>(null);
  const { busy, run } = useAction();
  const reload = () => api.loadOrder().then(setV).catch(() => setV({ rows: [], tool: false, dirty: false, last: null }));
  useEffect(() => {
    reload();
  }, [libraryRev]);

  if (!v) return null;
  const move = (i: number, d: -1 | 1) =>
    run("move", async () => {
      const order = v.rows.map((r) => r.folder);
      const j = i + d;
      if (j < 0 || j >= order.length) return;
      [order[i], order[j]] = [order[j], order[i]];
      await api.setLoadOrder(order);
      await reload();
    });
  const last = v.last;
  const tone = last && !last.ok ? "bad" : v.dirty ? "hot" : null;

  return (
    <div className="loadorder">
      <Deck>
        <Key icon="refresh" onClick={reload}>
          Re-read mods folder
        </Key>
      </Deck>
      <Module title="REDmods" icon="order" count={v.rows.length} bodyClassName="list-body" band={last && !last.ok ? "bad" : undefined}>
        {v.rows.length === 0 ? (
          <Empty icon="order" title="No REDmods installed">Mods that ship as REDmods (a folder with info.json under mods\) are ordered here.</Empty>
        ) : (
          <ol className="rows lo">
            {v.rows.map((r, i) => (
              <li key={r.folder} className="row mrow lorow">
                <span className="lcd-chip">
                  <b>{String(i + 1).padStart(2, "0")}</b>
                </span>
                <div className="m-main">
                  <span className="m-t">{r.name}</span>
                  <span className="m-meta">
                    <span className="mono">mods\{r.folder}</span>
                    {r.version ? <span>v{r.version}</span> : null}
                    <span>{r.owner ? `from ${r.owner}` : "installed outside Pocket Mods"}</span>
                  </span>
                </div>
                <span />
                <div className="m-due actions sm">
                  <Key size="sm" icon="up" aria-label={`Load ${r.name} earlier`} disabled={i === 0} onClick={() => move(i, -1)} />
                  <Key size="sm" icon="down" aria-label={`Load ${r.name} later`} disabled={i === v.rows.length - 1} onClick={() => move(i, 1)} />
                </div>
              </li>
            ))}
          </ol>
        )}
        <p className="note pad">First in the list loads first; when two REDmods change the same thing, the later one wins.</p>
      </Module>
      <Module title="Deploy" icon="play" className="lo-deploy" auto>
        <div className="lcds">
          <Lcd label="State" value={!v.tool ? "NO TOOL" : last && !last.ok ? "FAILED" : v.dirty ? "CHANGED" : "READY"} tone={!v.tool ? "bad" : tone} sub={last ? `last run ${date(last.at)}` : "never deployed"} size="sm" />
        </div>
        {!v.tool ? (
          <p className="note">The REDmod tool isn't installed. Add the free REDmod DLC: in Steam, the game's Properties, DLC; in GOG Galaxy, the game's DLC list.</p>
        ) : null}
        <div className="actions">
          <Key
            variant="primary"
            size="lg"
            icon="play"
            disabled={!v.tool || v.rows.length === 0 || busy === "deploy"}
            onClick={() =>
              run("deploy", async () => {
                const out = await api.redmodDeploy();
                toast(out.ok ? "REDmods deployed." : `REDmod deploy failed: ${out.problems[0] ?? "see the log"}`, out.ok ? undefined : { tone: "bad" });
                await reload();
                refresh();
              })
            }
          >
            {busy === "deploy" ? "Deploying…" : "Deploy REDmods"}
          </Key>
          {v.dirty ? <Pill tone="soon">Order or mods changed</Pill> : null}
        </div>
        {last?.problems.length ? (
          <>
            <div className="sect">Problems</div>
            <ul className="reqs">
              {last.problems.map((p) => (
                <li key={p} className="badc">
                  {p}
                </li>
              ))}
            </ul>
          </>
        ) : null}
        {last?.logTail.length ? (
          <>
            <div className="sect">Tool output</div>
            <pre className="lcd-log">{last.logTail.join("\n")}</pre>
          </>
        ) : null}
      </Module>
    </div>
  );
}
