// mod.io, read-only: a list of Cyberpunk 2077 mods on mod.io. The game installs them from its own Mods menu.
import { useEffect, useState } from "react";
import { Empty, Module, Pill, SearchField, bytesText, formatCount } from "../ui/pocket";
import { Icon } from "../ui/Icon";
import { errorText, extras, type ModioBrowse } from "../lib/api";

export function Modio() {
  const [text, setText] = useState("");
  const [q, setQ] = useState("");
  const [r, setR] = useState<ModioBrowse | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(true);
  useEffect(() => { const t = setTimeout(() => setQ(text), 400); return () => clearTimeout(t); }, [text]);
  useEffect(() => {
    // only the newest search lands
    let live = true;
    setBusy(true);
    extras.modioBrowse(q, 0).then((x) => { if (live) { setR(x); setErr(null); } }).catch((e) => { if (live) setErr(errorText(e)); })
      .finally(() => { if (live) setBusy(false); });
    return () => { live = false; };
  }, [q]);
  return (
    <div className="stack">
      <SearchField value={text} onChange={setText} placeholder="Search mod.io" />
      <Module icon="web" title="mod.io" count={r?.page ? `${formatCount(r.page.result_total)} · read-only` : "read-only"} bodyClass="list">
        {err && <div className="note bad" style={{ margin: ".6em" }}>{err}</div>}
        {r && !r.game && <Empty icon="web" title="Cyberpunk 2077 isn't on mod.io for this key" />}
        {r?.game && <div className="note" style={{ margin: ".6em" }}>Install these in game from Main menu, Mods</div>}
        {!r && busy && <div className="stack" style={{ gap: ".55em", padding: ".2em .8em .8em" }} aria-hidden="true">
          {Array.from({ length: 4 }, (_, i) => <span key={i} className="skel" style={{ height: "3.4em" }} />)}
        </div>}
        {r?.game && r.page && r.page.data.length === 0 && !busy && <Empty icon="browse" title="No mods match">Try other words</Empty>}
        <div className="rows">
          {r?.page?.data.map((m) => (
            <div key={m.id} className="row modrow" style={{ cursor: "default" }}>
              {m.logo.thumb_320x180 ? <img className="thumb" src={m.logo.thumb_320x180} alt="" loading="lazy" /> : <span className="thumb ph-img"><Icon name="file" /></span>}
              <span className="m-main"><span className="m-t">{m.name}</span><span className="m-meta"><span>{m.submitted_by.username}</span><span>{m.summary}</span></span></span>
              <span className="m-side"><span>{m.modfile ? bytesText(m.modfile.filesize) : ""}</span><span>↓ {formatCount(m.stats.downloads_total)}</span><Pill>mod.io</Pill></span>
            </div>
          ))}
        </div>
      </Module>
    </div>
  );
}
