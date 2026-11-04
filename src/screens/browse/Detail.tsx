// A mod's detail: picture, LCDs, console commands, files, requirements, the indexed file list, description, and the
// screen's one orange key, Install.
import { useEffect, useState } from "react";
import { Icon } from "../../ui/Icon";
import { Empty, Key, Lcd, Module, Tag, bytesText, formatCount } from "../../ui/pocket";
import { FRAMEWORKS, browse, descriptionMedia, endorseMod, errorText, fileBytes, trackMod, type CardView, type Classification, type DetailView } from "../../lib/api";
import { descriptionText, shortDate } from "../../lib/text";
import { extractCommands } from "../../lib/commands";
import { toast } from "../../ui/toast";
import { Gallery } from "./Media";

export function TypeTag({ c }: { c: Classification }) {
  const label = c.modType === "redmod" ? "REDmod" : c.modType === "unknown" ? "Type unknown" : c.modType[0].toUpperCase() + c.modType.slice(1);
  return <Tag kind={c.modType === "unknown" || c.modType === "mixed" ? undefined : c.modType} square>{label}</Tag>;
}

export function FrameworkTags({ c }: { c: Classification }) {
  return <>{c.frameworks.map((f) => <Tag key={f.key} kind={f.key}>{FRAMEWORKS.find((x) => x.key === f.key)?.short ?? f.name}</Tag>)}</>;
}

export function Detail({ card, onFlag, onInstall, onOpenPage, onOpen, onClose }: {
  card: CardView; onFlag: (c: CardView, f: "hidden" | "ignored", on: boolean) => void;
  onInstall?: (d: DetailView, fileId: number) => void; onOpenPage?: (modId: number) => void; onOpen: (modId: number) => void; onClose?: () => void;
}) {
  const [d, setD] = useState<DetailView | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [fileId, setFileId] = useState<number | null>(null);
  const [full, setFull] = useState(false);
  const [showList, setShowList] = useState(false);

  useEffect(() => {
    let live = true;
    setD(null); setErr(null); setFull(false); setShowList(false);
    browse.detail(card.modId, card.updatedAt || undefined).then((v) => {
      if (!live) return;
      setD(v);
      const main = v.files.filter((f) => f.category === "MAIN").sort((a, b) => b.date - a.date)[0] ?? v.files.filter((f) => ["MAIN", "OPTIONAL", "UPDATE", "MISCELLANEOUS"].includes(f.category)).sort((a, b) => b.date - a.date)[0];
      setFileId(main?.fileId ?? null);
    }).catch((e) => live && setErr(errorText(e)));
    return () => { live = false; };
  }, [card.modId, card.updatedAt]);

  if (err) return <Module icon="warn" title="Mod" band="bad" fill><div className="note bad">{err}</div></Module>;
  if (!d) return <Module icon="info" title="Mod" count="loading" fill><Empty icon="clock" title="Loading" /></Module>;
  const m = d.detail;
  const current = d.files.filter((f) => ["MAIN", "OPTIONAL", "UPDATE", "MISCELLANEOUS"].includes(f.category)).sort((a, b) => b.date - a.date);
  const file = d.files.find((f) => f.fileId === fileId);
  const desc = descriptionText(m.description);
  const commands = extractCommands(desc);
  const shots = descriptionMedia(m.description, m.pictureUrl);
  const view: CardView = { ...card, hidden: d.hidden, ignored: d.ignored };

  return (
    <Module icon="library" title={m.name} count={`v${m.version}`} fill className="detail-mod"
      tools={<>
        <Key size="sm" variant="ghost" icon="star" onClick={() => endorseMod(m.modId, m.version).then(() => toast(`Endorsed ${m.name}. Thanks from its author.`)).catch((e) => toast(errorText(e), true))} title="Endorse on Nexus" aria-label="Endorse" />
        <Key size="sm" variant="ghost" icon="eye" onClick={() => trackMod(m.modId).then(() => toast(`Tracking ${m.name} on Nexus.`)).catch((e) => toast(errorText(e), true))} title="Track on Nexus: its updates show in your Nexus feed" aria-label="Track" />
        <Key size="sm" variant="ghost" icon="web" onClick={() => onOpenPage?.(m.modId)} title="Open the Nexus page in the app" aria-label="Open the Nexus page" />
        {onClose && <Key size="sm" variant="ghost" icon="x" onClick={onClose} aria-label="Close" />}
      </>}>
      {shots.length > 0 ? <Gallery items={shots} /> : m.pictureUrl && <img className="hero-img" src={m.pictureUrl} alt="" />}
      <div className="m-meta" style={{ marginTop: ".6em" }}><span>{m.author ?? m.uploader?.name}</span><span>{m.category}</span><span>Updated {shortDate(m.updatedAt)}</span></div>
      <p style={{ margin: ".5em 0 .2em", color: "var(--text)" }}>{m.summary}</p>
      <div className="m-tags"><TypeTag c={d.classification} /><FrameworkTags c={d.classification} /></div>
      {commands.length > 0 && <Commands list={commands} />}

      <div className="lcds" style={{ marginTop: ".8em" }}>
        <Lcd size="sm" label="Downloads" value={formatCount(m.downloads)} />
        <Lcd size="sm" label="Endorsed" value={formatCount(m.endorsements)} />
        {file && <Lcd size="sm" label="Size" value={bytesText(fileBytes(file)).split(" ")[0]} unit={bytesText(fileBytes(file)).split(" ")[1]} />}
      </div>

      <div className="sub-h">File</div>
      {current.length === 0 ? <p className="dim">No current files. Nexus may have removed them.</p> : (
        <div className="rows">
          {current.slice(0, 8).map((f) => (
            <button key={f.fileId} type="button" className={`row${f.fileId === fileId ? " sel" : ""}`} onClick={() => setFileId(f.fileId)}>
              <span className="m-main"><span className="m-t">{f.name}</span><span className="m-meta"><span>{f.category.toLowerCase().replace("_", " ")}</span><span>v{f.version}</span><span>{shortDate(new Date(f.date * 1000).toISOString())}</span></span></span>
              <span className="m-side"><span>{bytesText(fileBytes(f))}</span></span>
            </button>
          ))}
        </div>
      )}

      <div className="actions" style={{ marginTop: ".8em" }}>
        <Key variant="primary" size="lg" icon="install" disabled={!file} onClick={() => file && onInstall?.(d, file.fileId)}>Install</Key>
        <Key variant="ghost" icon="hide" onClick={() => { onFlag(view, "hidden", !d.hidden); setD({ ...d, hidden: !d.hidden }); }}>{d.hidden ? "Unhide" : "Hide"}</Key>
        <Key variant="ghost" icon="x" onClick={() => { onFlag(view, "ignored", !d.ignored); setD({ ...d, ignored: !d.ignored }); }}>{d.ignored ? "Stop ignoring" : "Ignore"}</Key>
      </div>

      {m.modRequirements.nexusRequirements.totalCount > 0 && <>
        <div className="sub-h">Requires</div>
        <div className="rows">
          {m.modRequirements.nexusRequirements.nodes.map((r) => (
            <button key={`${r.modId}-${r.modName}`} type="button" className="row" onClick={() => !r.externalRequirement && onOpen(r.modId)} disabled={r.externalRequirement}>
              <span className="m-main"><span className="m-t">{r.modName}</span>{r.notes && <span className="m-meta"><span>{r.notes}</span></span>}</span>
              <span className="m-side">{r.externalRequirement ? <span>Off Nexus</span> : <Icon name="chevron" size={9} />}</span>
            </button>
          ))}
        </div>
      </>}

      <div className="sub-h">Files in the archive</div>
      {d.fileList ? (
        <>
          <p className="dim" style={{ fontSize: ".86em" }}>{d.fileList.length} paths indexed by Nexus{d.classification.typeSource === "files" ? ", used for the type" : ""}.{" "}
            <button type="button" className="linkish" onClick={() => setShowList(!showList)}>{showList ? "Hide" : "Show"}</button></p>
          {showList && <div className="log" style={{ maxHeight: "12em" }}>{d.fileList.slice(0, 200).join("\n")}</div>}
        </>
      ) : <p className="dim" style={{ fontSize: ".86em" }}>Nexus has no file list for this file yet. The type is a guess until it is downloaded.</p>}

      <div className="sub-h">Description</div>
      <div className="desc">{full ? desc : desc.slice(0, 900)}{desc.length > 900 && !full && "…"}</div>
      {desc.length > 900 && <Key size="sm" variant="ghost" onClick={() => setFull(!full)}>{full ? "Less" : "Read more"}</Key>}
    </Module>
  );
}

/** Console commands from the description, at the top of the detail in a dropdown that opens. */
export function Commands({ list }: { list: Array<{ command: string; generated: boolean }> }) {
  const [open, setOpen] = useState(list.length <= 3);
  const copy = (t: string) => navigator.clipboard.writeText(t).then(() => toast("Copied. Paste it in the Cyber Engine Tweaks console in game.")).catch(() => toast("Could not copy.", true));
  return (
    <div className="cmds">
      <button type="button" className="cmds-h" aria-expanded={open} onClick={() => setOpen(!open)}>
        <Icon name="chevron" size={9} className={open ? "rot" : ""} />
        <b>Console commands</b><span className="n">{list.length}</span>
        <span className="sp" />
        <span className="dim">Cyber Engine Tweaks console</span>
      </button>
      {open && <>
        <div className="cmds-list">
          {list.map((c) => (
            <div key={c.command} className="cmd">
              <code>{c.command}</code>
              {c.generated && <span className="dim" title="Made from an item ID in the description">from item ID</span>}
              <Key size="sm" variant="ghost" icon="file" aria-label={`Copy ${c.command}`} title="Copy" onClick={() => copy(c.command)} />
            </div>
          ))}
        </div>
        {list.length > 1 && <Key size="sm" icon="file" onClick={() => copy(list.map((c) => c.command).join("\n"))}>Copy all</Key>}
      </>}
    </div>
  );
}
