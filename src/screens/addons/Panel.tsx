// An extension's panel: its page in a sandboxed frame. In the desktop app the page comes from the pmext protocol
// (src-tauri/src/commands/extensions.rs) with its own strict CSP; in the browser demo the same page is built here
// and given as srcdoc. Either way the frame has sandbox="allow-scripts" and no allow-same-origin, so it runs in an
// opaque origin: it cannot read the app, its storage or Tauri, and it only talks to the app by postMessage.
import { useEffect, useMemo, useRef, useState } from "react";
import { Dialog } from "../../ui/Dialog";
import { Icon } from "../../ui/Icon";
import { Key, Module, bytesText } from "../../ui/pocket";
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
  const [ask, setAsk] = useState<{ modId: number; fileId: number | null; done: (yes: boolean | "block") => void } | null>(null);
  const [stopped, setStopped] = useState(false);
  const [showPerms, setShowPerms] = useState(false);
  const info: PanelInfo = useMemo(() => ({ extId: panel.extId, extName: panel.extName, release: panel.release, panelId: panel.id, title: panel.title, permissions: panel.permissions }),
    [panel.extId, panel.extName, panel.release, panel.id, panel.title, panel.permissions]);

  useEffect(() => { if (inTauri()) panelUrl(panel).then(setSrc).catch(() => setSrc(null)); }, [panel]);

  useEffect(() => {
    const handle = makeBridge(info, {
      toast,
      confirmQueue: (modId, fileId) => new Promise<boolean | "block">((done) => setAsk({ modId, fileId, done })),
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
            if (++flood > 200) { setStopped(true); toast(`${panel.extName}'s panel was stopped: it sent too many requests. You can remove it in Add-ons, Installed.`, true); }
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
          {panel.permissions.length === 0 ? <span>This panel has no permissions: it can only show things.</span>
            : panel.permissions.map((p) => <span key={p} className="pill"><Icon name="key" size={9} />{PERMISSIONS[p]?.title ?? p}</span>)}
          <span className="dim">Sandboxed: no web requests, no files, no access to the app beyond these.</span>
        </div>
      )}
      {stopped && <div className="note warn" style={{ margin: ".8em" }}>This panel was stopped because it sent too many requests. Remove or turn off its extension in Add-ons, Installed.</div>}
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
  const [busy, setBusy] = useState(false);
  useEffect(() => { browse.detail(modId).then(setD).catch((e) => setErr(errorText(e))); }, [modId]);
  const file = d ? (fileId ? d.files.find((f) => f.fileId === fileId) : d.files.filter((f) => f.category === "MAIN").sort((a, b) => b.date - a.date)[0] ?? d.files[0]) : undefined;
  const yes = async () => {
    if (!d || !file) return;
    setBusy(true);
    try { await queue.add(modId, file.fileId); toast(`${d.detail.name} is queued.`); onClose(true); } catch (e) { toast(errorText(e), true); onClose(false); } finally { setBusy(false); }
  };
  return (
    <Dialog icon="downloads" title="Add to the download queue?" onClose={() => onClose(false)}>
      <p><b className="hi">{extName}</b> wants to add a mod to your downloads.</p>
      {err && <div className="note bad">{err}</div>}
      {d && (
        <div className="ext-ask">
          <AddonGlyph icon="downloads" size={27} />
          <span className="m-main">
            <span className="m-t">{d.detail.name}</span>
            <span className="m-meta"><span>{d.detail.author ?? d.detail.uploader?.name}</span>{file && <span>{file.name}</span>}{file && <span>{bytesText(fileBytes(file))}</span>}</span>
          </span>
        </div>
      )}
      {d && !file && <div className="note warn">This mod has no file to download.</div>}
      <p className="dim" style={{ fontSize: ".86em" }}>Required mods come along as usual. Nothing is downloaded unless you say yes.</p>
      <div className="actions" style={{ justifyContent: "flex-end" }}>
        <Key variant="ghost" onClick={() => onClose("block")} title="It cannot ask again until Pocket Mods restarts">Block its requests</Key>
        <Key variant="ghost" onClick={() => onClose(false)}>No</Key>
        <Key variant="primary" icon="downloads" disabled={!file || busy} onClick={yes}>Add to downloads</Key>
      </div>
    </Dialog>
  );
}
