// Before anything downloads: what an install brings in (the mod and its missing requirements), how big it is and,
// for free accounts, how many clicks on Nexus it will take.
import { useEffect, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Empty, Key, Lcd, Pill, bytesText } from "../ui/pocket";
import { errorText, queue, type QueueItem } from "../lib/api";
import { toast } from "../ui/toast";
import { Switch } from "../ui/pocket";
import { installPrefs, setInstallPrefs } from "../lib/prefs";
import { PremiumNote } from "../ui/Premium";

export function InstallDialog({ modId, fileId, name, premium, onClose, onQueued }: {
  modId: number; fileId: number; name: string; premium: boolean; onClose: () => void; onQueued: () => void;
}) {
  const [items, setItems] = useState<QueueItem[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [withReqs, setWithReqs] = useState(true);
  const [dontAsk, setDontAsk] = useState(false);
  useEffect(() => { queue.plan(modId, fileId).then(setItems).catch((e) => setErr(errorText(e))); }, [modId, fileId]);
  useEffect(() => { installPrefs().then((p) => setWithReqs(p.requirements !== "never")).catch(() => {}); }, []);

  const chosen = items ? (withReqs ? items : items.filter((i) => i.modId === modId)) : null;
  const reqCount = items ? items.filter((i) => i.modId !== modId).length : 0;
  const clicks = chosen ? (premium ? chosen.filter((i) => i.manualOnly).length : chosen.length) : 0;
  const bytes = chosen?.reduce((a, i) => a + i.size, 0) ?? 0;
  const go = async () => {
    setBusy(true);
    try {
      if (dontAsk) await setInstallPrefs({ confirm: false, requirements: withReqs ? "always" : "never" });
      await queue.add(modId, fileId, withReqs);
      toast(clicks ? `Queued. ${clicks} ${clicks === 1 ? "mod needs" : "mods need"} a click on Nexus.` : "Queued. It installs by itself.");
      onQueued();
    } catch (e) { setErr(errorText(e)); setBusy(false); }
  };

  return (
    <Dialog icon="install" title={`Install ${name}`} onClose={onClose}>
      {err && <div className="note bad">{err}</div>}
      {!items && !err && <Empty icon="clock" title="Checking requirements" />}
      {items && chosen && <>
        <div className="lcds">
          <Lcd size="sm" label="Mods" value={String(chosen.length).padStart(2, "0")} sub={reqCount ? (withReqs ? `with ${reqCount} required` : "requirements left out") : "no missing requirements"} />
          <Lcd size="sm" label="Needs your click" value={String(clicks).padStart(2, "0")} tone={clicks ? "hot" : "ok"} sub={premium ? "Premium: automatic" : "Free account"} />
          <Lcd size="sm" label="Download" value={bytesText(bytes).split(" ")[0]} unit={bytesText(bytes).split(" ")[1]} />
        </div>
        {!premium && clicks > 0 && <div className="note warn">Free accounts press "Mod Manager Download" once per file. Each page opens in turn and the queue moves on by itself.</div>}
        {!premium && <PremiumNote clicks={clicks} />}
        <div className="rows">
          {chosen.map((i) => (
            <div key={`${i.modId}-${i.fileId}`} className="row">
              <span className="m-main"><span className="m-t">{i.modName}</span>
                <span className="m-meta"><span>{i.fileName}</span><span>v{i.version}</span>{i.requiredBy && <span>needed by {i.requiredBy}</span>}</span></span>
              <span className="m-side"><span>{bytesText(i.size)}</span>{i.manualOnly && <Pill tone="soon">Manual</Pill>}</span>
            </div>
          ))}
        </div>
        <div className="stack" style={{ gap: ".4em" }}>
          {reqCount > 0 && <Switch checked={withReqs} onChange={setWithReqs}>Also install the {reqCount} required {reqCount === 1 ? "mod" : "mods"}</Switch>}
          {!withReqs && reqCount > 0 && <div className="note warn">Without its requirements this mod may not work or may stop the game from starting.</div>}
          <Switch checked={dontAsk} onChange={setDontAsk}>Next time, install straight away with these choices</Switch>
        </div>
        <div className="actions" style={{ justifyContent: "flex-end" }}>
          <Key variant="ghost" onClick={onClose}>Cancel</Key>
          <Key variant="primary" icon="install" onClick={go} disabled={busy}>{chosen.length > 1 ? `Install ${chosen.length} mods` : "Install"}</Key>
        </div>
      </>}
    </Dialog>
  );
}
