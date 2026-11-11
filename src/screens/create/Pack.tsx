// Check a mod folder or archive the way players' managers will see it, and pack it into a clean zip.
import { Key, Lcd, Module, Pill, Tag, useProgress } from "../../ui/pocket";
import { creator } from "../../lib/api";
import { plural } from "../../lib/text";
import { fileName, type Pack } from "./shared";

export function CheckModule({ pack, compact }: { pack: Pack; compact?: boolean }) {
  const c = pack.check;
  return (
    <Module icon="file" title="Check your mod" count={c ? plural(c.files, "file") : pack.busy ? "checking" : undefined}>
      <div className="actions">
        <Key icon="folder" onClick={() => void pack.pick(true)} busy={pack.busy === "folder"} disabled={!!pack.busy}>Choose a folder</Key>
        <Key variant="ghost" icon="file" onClick={() => void pack.pick(false)} busy={pack.busy === "archive"} disabled={!!pack.busy}>Choose an archive</Key>
      </div>
      {pack.path && <p className="dim mono" style={{ fontSize: ".78em", marginTop: ".5em", overflowWrap: "anywhere" }}>{pack.path}</p>}
      {c && <>
        <div className="lcds" style={{ marginTop: ".6em" }}>
          <Lcd size="sm" label="Installs" value={String(c.plan.files.length).padStart(2, "0")} sub={c.plan.modType ?? "type unknown"} />
          <Lcd size="sm" label="Problems" value={String(c.problems.length).padStart(2, "0")} tone={c.problems.length ? "bad" : "ok"} />
        </div>
        {c.problems.map((p) => <div key={p} className="note bad" style={{ marginTop: ".4em" }}>{p}</div>)}
        {c.tips.map((p) => <div key={p} className="note warn" style={{ marginTop: ".4em" }}>{p}</div>)}
        {c.problems.length === 0 && <div className="note" style={{ marginTop: ".4em" }}>Ready, every file has a place</div>}
        {c.needs.length > 0 && <>
          <div className="sub-h">Needs</div>
          <div className="m-tags">{c.needs.map((n) => <Tag key={n.key} kind={n.key}>{n.name}</Tag>)}</div>
        </>}
        {!compact && <>
          <div className="sub-h">Where the files go</div>
          <div className="log" style={{ maxHeight: "12em" }}>{c.plan.files.slice(0, 200).map((f) => f.target).join("\n") || "nothing"}</div>
        </>}
      </>}
    </Module>
  );
}

export function PackModule({ pack, primary }: { pack: Pack; primary?: boolean }) {
  const why = !pack.path ? "Choose your mod's folder first" : !pack.isFolder ? "Choose a folder to pack a clean copy"
    : !pack.check ? (pack.busy ? null : "Its check didn't finish, choose the folder again")
      : !pack.check.plan.files.length ? "Nothing to pack, no file has a place in the game" : null;
  const packed = useProgress(pack.busy === "pack", creator.onPack);
  return (
    <Module icon="install" title="Pack it" count={pack.isFolder && pack.zip ? "packed" : undefined}>
      <div className="fgrid">
        <span className="lab">Name</span><label className="fbox" style={{ flex: "none" }}><input className="finput" value={pack.name} onChange={(e) => pack.set({ name: e.target.value })} aria-label="Mod name" /></label>
        <span className="lab">Version</span><label className="fbox" style={{ flex: "none" }}><input className="finput" value={pack.version} onChange={(e) => pack.set({ version: e.target.value })} aria-label="Version" /></label>
      </div>
      <div className="actions" style={{ marginTop: ".5em" }}>
        <Key variant={primary ? "primary" : undefined} icon="install" title={why ?? "Files at their game paths, docs left out, saved next to the folder"}
          busy={pack.busy === "pack" && (packed ?? true)} disabled={!!why || !!pack.busy || !pack.check} onClick={() => void pack.pack()}>Pack a clean zip</Key>
        {pack.zip && <Pill tone="ok">{fileName(pack.zip)}</Pill>}
      </div>
      {pack.path && !pack.isFolder && <p className="dim hint">Choose a folder to pack a clean copy</p>}
    </Module>
  );
}
