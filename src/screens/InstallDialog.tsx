// Before anything downloads: what an install brings in (the mod and its missing requirements), how big it is and,
// for free accounts, how many clicks on Nexus it will take.
import { useEffect, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Empty, Key, Lcd, Pill, bytesText } from "../ui/pocket";
import { errorText, queue, type QueueItem } from "../lib/api";
import { toast } from "../ui/toast";

export function InstallDialog({ modId, fileId, name, premium, onClose, onQueued }: {
  modId: number; fileId: number; name: string; premium: boolean; onClose: () => void; onQueued: () => void;
}) {
  const [items, setItems] = useState<QueueItem[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => { queue.plan(modId, fileId).then(setItems).catch((e) => setErr(errorText(e))); }, [modId, fileId]);

  const clicks = items ? (premium ? items.filter((i) => i.manualOnly).length : items.length) : 0;
  const bytes = items?.reduce((a, i) => a + i.size, 0) ?? 0;
  const go = async () => {
    setBusy(true);
    try {
      await queue.add(modId, fileId, true);
      toast(clicks ? `Queued. ${clicks} ${clicks === 1 ? "mod needs" : "mods need"} a click on Nexus.` : "Queued. It installs by itself.");
      onQueued();
    } catch (e) { setErr(errorText(e)); setBusy(false); }
  };

  return (
    <Dialog icon="install" title={`Install ${name}`} onClose={onClose}>
      {err && <div className="note bad">{err}</div>}
      {!items && !err && <Empty icon="clock" title="Checking requirements" />}
      {items && <>
        <div className="lcds">
          <Lcd size="sm" label="Mods" value={String(items.length).padStart(2, "0")} sub={items.length > 1 ? "with requirements" : "no missing requirements"} />
          <Lcd size="sm" label="Needs your click" value={String(clicks).padStart(2, "0")} tone={clicks ? "hot" : "ok"} sub={premium ? "Premium: automatic" : "Free account"} />
          <Lcd size="sm" label="Download" value={bytesText(bytes).split(" ")[0]} unit={bytesText(bytes).split(" ")[1]} />
        </div>
        {!premium && clicks > 0 && <div className="note warn">Free accounts press "Mod Manager Download" once per file. Each page opens here in the app and the queue moves on by itself.</div>}
        <div className="rows">
          {items.map((i) => (
            <div key={`${i.modId}-${i.fileId}`} className="row">
              <span className="m-main"><span className="m-t">{i.modName}</span>
                <span className="m-meta"><span>{i.fileName}</span><span>v{i.version}</span>{i.requiredBy && <span>needed by {i.requiredBy}</span>}</span></span>
              <span className="m-side"><span>{bytesText(i.size)}</span>{i.manualOnly && <Pill tone="soon">Manual</Pill>}</span>
            </div>
          ))}
        </div>
        <div className="actions" style={{ justifyContent: "flex-end" }}>
          <Key variant="ghost" onClick={onClose}>Cancel</Key>
          <Key variant="primary" icon="install" onClick={go} disabled={busy}>{items.length > 1 ? `Install ${items.length} mods` : "Install"}</Key>
        </div>
      </>}
    </Dialog>
  );
}
