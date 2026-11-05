// Moving the mod library: pick a new folder, see what the move does (a quick rename on the same drive, a copy
// onto another one), then move it. Every stored path moves with it and the game's files stay linked.
import { useEffect, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Key, Lcd, Pill, bytesText } from "../ui/pocket";
import { errorText, pickFolder, relocate, type MoveCheck, type RelinkReport } from "../lib/api";
import { toast } from "../ui/toast";

export function relinkText(r: RelinkReport): string {
  const parts = [
    r.relinked && `${r.relinked} linked to the library again`,
    r.restored && `${r.restored} put back`,
    r.copies && `${r.copies} kept as copies (other drive)`,
    r.changed && `${r.changed} changed in the game, left alone`,
  ].filter(Boolean);
  return parts.length ? `Game mod files: ${parts.join(", ")}` : `All ${r.checked} game mod files linked`;
}

export function MoveLibraryDialog({ onClose, onMoved }: { onClose: () => void; onMoved: () => void }) {
  const [check, setCheck] = useState<MoveCheck | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<[number, number] | null>(null);
  useEffect(() => { let off: (() => void) | undefined; relocate.onProgress(setProgress).then((u) => { off = u; }); return () => off?.(); }, []);

  const pick = async () => {
    const d = await pickFolder("New library folder (best on the game's drive)");
    if (!d) return;
    try { setCheck(await relocate.check(d)); } catch (e) { toast(errorText(e), true); }
  };
  // the folder picker opens with the dialog
  useEffect(() => { void pick(); }, []);

  const move = async () => {
    if (!check) return;
    setBusy(true);
    try {
      const r = await relocate.move(check.to);
      toast(r.how === "renamed" ? `Library moved to ${check.to}`
        : `Copied ${bytesText(r.bytes)} to ${check.to}${r.relinked ? `, ${r.relinked} game files relinked` : ""}${r.copyTurnedOn ? ". New mods will be copied" : ""}`);
      if (r.leftBehind) toast(`Couldn't delete part of the old folder: ${r.leftBehind}`, true);
      onMoved(); onClose();
    } catch (e) { toast(errorText(e), true); setBusy(false); }
  };

  return (
    <Dialog icon="folder" title="Move the mod library" onClose={busy ? () => {} : onClose} band={check?.problem ? "bad" : !check?.sameDriveAsGame && check ? "hot" : undefined}>
      {!check ? (
        <>
          <p>Pick a new or empty folder</p>
          <div className="actions" style={{ justifyContent: "flex-end" }}><Key variant="ghost" onClick={onClose}>Cancel</Key><Key icon="folder" onClick={pick}>Choose a folder</Key></div>
        </>
      ) : (
        <>
          <div className="fgrid">
            <span className="lab">From</span><code className="mvpath">{check.from}</code>
            <span className="lab">To</span><code className="mvpath">{check.to}</code>
          </div>
          <div className="lcds" style={{ margin: ".7em 0" }}>
            <Lcd size="sm" label="Files" value={String(check.files)} sub={bytesText(check.bytes)} />
            <Lcd size="sm" label="Move" value={check.quick ? "FAST" : "COPY"} tone={check.quick ? "ok" : "hot"} sub={check.quick ? "same drive" : "other drive"} />
            {progress && busy && <Lcd size="sm" label="Copied" value={`${Math.round((progress[0] / Math.max(progress[1], 1)) * 100)}`} unit="%" bar={progress[0] / Math.max(progress[1], 1)} />}
          </div>
          {check.problem ? <div className="note bad">{check.problem}</div>
            : check.sameDriveAsGame ? <div className="actions"><Pill tone="ok">Same drive as the game</Pill></div>
            : <div className="note warn">Different drive from the game, so mod files get copied and use twice the space</div>}
          <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".7em" }}>
            <Key variant="ghost" onClick={onClose} disabled={busy}>Cancel</Key>
            <Key variant="ghost" icon="folder" onClick={pick} disabled={busy}>Another folder</Key>
            <Key variant="primary" icon="folder" onClick={move} disabled={busy || !!check.problem}>{busy ? "Moving" : check.sameDriveAsGame ? "Move library" : "Move and copy"}</Key>
          </div>
        </>
      )}
    </Dialog>
  );
}
