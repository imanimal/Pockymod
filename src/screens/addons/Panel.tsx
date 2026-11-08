// An extension's panel: its page in a sandboxed frame. In the desktop app the page comes from the pmext protocol
// (src-tauri/src/commands/extensions.rs) with its own strict CSP; in the browser demo the same page is built here
// and given as srcdoc. Either way the frame has sandbox="allow-scripts" and no allow-same-origin, so it runs in an
// opaque origin: it cannot read the app, its storage or Tauri, and it only talks to the app by postMessage.
import { useEffect, useMemo, useRef, useState } from "react";
import { Dialog } from "../../ui/Dialog";
import { Icon } from "../../ui/Icon";
import { Key, Module, bytesText, useBusy } from "../../ui/pocket";
import { toast } from "../../ui/toast";
import { browse, errorText, fileBytes, queue, type DetailView } from "../../lib/api";
import { inTauri } from "../../lib/tauri";
import { PanelError, TOO_MANY, makeBridge, panelTheme, readCall, type PanelInfo } from "../../lib/panel-bridge";
import { PERMISSIONS } from "../../lib/addon-rules.mjs";
import type { Contributions } from "../../lib/extensions";
import shell from "../../../src-tauri/src/commands/panel-shell.html?raw";
import { AddonGlyph } from "./parts";

type Panel = Contributions["panels"][number];

async function panelUrl(p: Panel): Promise<string> {
  const { convertFileSrc } = await import("@tauri-apps/api/core");
  return `${convertFileSrc(`${p.extId}/${p.id}`, "pmext")}?r=${encodeURIComponent(p.release)}`;
}

export function PanelHost({ panel }: { panel: Panel }) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [src, setSrc] = useState<string | null>(null);
  const [srcErr, setSrcErr] = useState<string | null>(null);
  const [ask, setAsk] = useState<{ modId: number; fileId: number | null; done: (yes: boolean | "block") => void } | null>(null);
  const [stopped, setStopped] = useState(false);
  const [showPerms, setShowPerms] = useState(false);
  const info: PanelInfo = useMemo(() => ({ extId: panel.extId, extName: panel.extName, release: panel.release, panelId: panel.id, title: panel.title, permissions: panel.permissions }),
    [panel.extId, panel.extName, panel.release, panel.id, panel.title, panel.permissions]);

  useEffect(() => { if (inTauri()) panelUrl(panel).then((u) => { setSrc(u); setSrcErr(null); }).catch((e) => setSrcErr(errorText(e))); }, [panel]);

  useEffect(() => {
    const handle = makeBridge(info, {
      toast,
      // a newer question replaces one still open, which then counts as a no, so the panel never waits forever
      confirmQueue: (modId, fileId) => new Promise<boolean | "block">((done) => setAsk((was) => { was?.done(false); return { modId, fileId, done }; })),
    });
    let flood = 0;
    const post = (msg: unknown) => frame.current?.contentWindow?.postMessage(msg, "*");
    const onMessage = (e: MessageEvent) => {
      // only this frame's window; its origin is "null" (opaque), so the window itself is the check
      if (!frame.current || e.source !== frame.current.contentWindow) return;
      const c = readCall(e.data);
      if (!c) return;
      handle(c.method, c.params)
        .then((result) => post({ pocket: 1, id: c.id, result: result === undefined ? null : result }))
        .catch((err) => {
          // calls over the limit get no answer; a panel that keeps flooding is stopped
          if (err instanceof PanelError && err.message === TOO_MANY) {
            if (++flood > 200) { setStopped(true); toast(`Stopped ${panel.extName}'s panel, too many requests`, true); }
            return;
          }
          post({ pocket: 1, id: c.id, error: err instanceof PanelError ? err.message : errorText(err) });
        });
    };
    window.addEventListener("message", onMessage);
    // the theme can change while the panel is open
    const watch = new MutationObserver(() => post({ pocket: 1, event: "theme", data: panelTheme() }));
    watch.observe(document.documentElement, { attributes: true, attributeFilter: ["class", "style", "data-scheme", "data-theme"] });
    return () => { window.removeEventListener("message", onMessage); watch.disconnect(); };
  }, [info]);

  const demoDoc = useMemo(() => (inTauri() ? undefined : shell.replace("<!--PANEL-->", () => panel.html)), [panel.html]);

  return (
    <Module icon="addons" title={panel.title} count={panel.extName} fill className="ext-panel" bodyClass="flush"
      tools={<Key size="sm" variant="ghost" icon="key" latched={showPerms} onClick={() => setShowPerms(!showPerms)} title="What this panel may do">Permissions</Key>}>
      {showPerms && (
        <div className="ext-panel-perms">
          {panel.permissions.length === 0 ? <span>No permissions, display only</span>
            : panel.permissions.map((p) => <span key={p} className="pill"><Icon name="key" size={9} />{PERMISSIONS[p]?.title ?? p}</span>)}
          <span className="dim">Sandboxed. No web, no files</span>
        </div>
      )}
      {stopped && <div className="note warn" style={{ margin: ".8em" }}>Stopped, too many requests</div>}
      {srcErr && !src && <div className="note bad" style={{ margin: ".8em" }}>Couldn't open this panel ({srcErr})</div>}
      {!stopped && (src || demoDoc !== undefined) && (
        <iframe ref={frame} className="ext-frame" title={`${panel.title} (${panel.extName})`} sandbox="allow-scripts" referrerPolicy="no-referrer" allow=""
          src={src ?? undefined} srcDoc={demoDoc} />
      )}
      {ask && <QueueAsk extName={panel.extName} {...ask} onClose={(yes) => { ask.done(yes); setAsk(null); }} />}
    </Module>
  );
}

/** A panel asked to queue a download: the user says yes or no, every time. */
function QueueAsk({ extName, modId, fileId, onClose }: { extName: string; modId: number; fileId: number | null; onClose: (yes: boolean | "block") => void }) {
  const [d, setD] = useState<DetailView | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, run] = useBusy();
  useEffect(() => {
    let live = true;
    setD(null); setErr(null);
    browse.detail(modId).then((v) => { if (live) setD(v); }).catch((e) => { if (live) setErr(errorText(e)); });
    return () => { live = false; };
  }, [modId]);
  const file = d ? (fileId ? d.files.find((f) => f.fileId === fileId) : d.files.filter((f) => f.category === "MAIN").sort((a, b) => b.date - a.date)[0] ?? d.files[0]) : undefined;
  const yes = () => run(async () => {
    if (!d || !file) return;
    await queue.add(modId, file.fileId);
    toast(`${d.detail.name} queued`);
    onClose(true);
  });
  return (
    <Dialog icon="downloads" title="Add to the download queue?" onClose={() => onClose(false)}>
      <p><b className="hi">{extName}</b> wants to add a mod to your downloads</p>
      {err && <div className="note bad">{err}</div>}
      {!d && !err && (
        <div className="ext-ask" aria-hidden="true">
          <span className="skel" style={{ width: "2.2em", height: "2.2em", flex: "none" }} />
          <span className="m-main" style={{ flex: 1, gap: ".35em" }}><span className="skel skel-line" style={{ width: "60%" }} /><span className="skel skel-line" style={{ width: "85%" }} /></span>
        </div>
      )}
      {d && (
        <div className="ext-ask">
          <AddonGlyph icon="downloads" size={27} />
          <span className="m-main">
            <span className="m-t">{d.detail.name}</span>
            <span className="m-meta"><span>{d.detail.author ?? d.detail.uploader?.name}</span>{file && <span>{file.name}</span>}{file && <span>{bytesText(fileBytes(file))}</span>}</span>
          </span>
        </div>
      )}
      {d && !file && <div className="note warn">No file to download</div>}
      <div className="actions" style={{ justifyContent: "flex-end" }}>
        <Key variant="ghost" onClick={() => onClose("block")} title="Until Pockymod restarts">Block its requests</Key>
        <Key variant="ghost" onClick={() => onClose(false)}>No</Key>
        <Key variant="primary" icon="downloads" busy={busy} disabled={!file} onClick={() => void yes()}
          title={file ? undefined : d ? "This mod has no file to download" : err ? "Couldn't read the mod from Nexus" : undefined}>Add to downloads</Key>
      </div>
    </Dialog>
  );
}
