// The check before an extension does anything: what it adds and what it can do, shown before install, after an
// update that needs something new, and for a file the user imports. Nothing runs until the user says yes here.
import { suspendUserCss } from "../../lib/theme";
import { useState, useEffect } from "react";
import { Dialog } from "../../ui/Dialog";
import { Icon } from "../../ui/Icon";
import { Key, useBusy } from "../../ui/pocket";
import { toast } from "../../ui/toast";
import { errorText, pickProgram, type ExtState } from "../../lib/api";
import { approveExtension, installExtension, newApprovals, type ExtensionManifest } from "../../lib/extensions";
import { recordVersion } from "../../lib/market";
import { AddonGlyph, AddsList, PermissionList } from "./parts";

export type ReviewMode = "install" | "update" | "review";

export function ReviewDialog({ manifest: m, text, mode, state, fromMarket, warnings = [], onClose, onDone }: {
  manifest: ExtensionManifest; text: string; mode: ReviewMode; state?: ExtState | null; fromMarket?: boolean; warnings?: string[];
  onClose: () => void; onDone?: (m: ExtensionManifest) => void;
}) {
  // a theme's own css stays out of this dialog, so nothing it shows can be hidden
  useEffect(() => { suspendUserCss(true); return () => suspendUserCss(false); }, []);
  const [paths, setPaths] = useState<Record<string, string>>(state?.toolPaths ?? {});
  const [busy, run] = useBusy();
  const fresh = mode === "install" ? undefined : newApprovals(m, state);
  const picks = (m.contributes.tools ?? []).filter((t) => t.pick);
  const hasPanels = (m.contributes.panels ?? []).length > 0;

  const choose = async (toolId: string, title: string) => {
    try {
      const p = await pickProgram(title);
      if (p) setPaths((x) => ({ ...x, [toolId]: p }));
    } catch (e) { toast(errorText(e), true); }
  };
  const go = () => run(async () => {
    if (mode === "review") await approveExtension(m.id, paths);
    else await installExtension(text, { approve: true, fromMarket, toolPaths: paths });
    // the version note only drives the "update available" badge, so a failure here is not worth a message
    if (fromMarket) await recordVersion("extension", m.id, m.release).catch(() => {});
    toast(mode === "update" ? `${m.name} updated to ${m.release}` : `${m.name} on${(m.contributes.tools?.length || m.contributes.launchOptions?.length) ? ". Tools are in the Play menu" : ""}`);
    onDone?.(m);
    onClose();
  });

  const title = mode === "install" ? "Install this extension?" : mode === "update" ? "This update needs your OK" : "Turn this extension on?";
  return (
    <Dialog icon="addons" title={title} count={`v${m.release}`} onClose={onClose}>
      <div className="ext-head">
        <span className="ext-glyph"><AddonGlyph icon={m.icon} size={36} /></span>
        <span className="m-main">
          <b className="hi ext-name">{m.name}</b>
          <span className="m-meta"><span>{m.author ?? "Unknown author"}</span><span>v{m.release}</span>{fromMarket && <span>From the marketplace</span>}</span>
        </span>
      </div>
      {m.description && <p>{m.description}</p>}
      {mode !== "install" && fresh && fresh.length > 0 && <div className="note warn"><b>New</b> means you haven't allowed it before</div>}
      {warnings.length > 0 && <div className="note">{warnings.slice(0, 3).map((w) => <div key={w}>{w}</div>)}</div>}

      <div className="ext-sec">
        <div className="ext-sec-h"><Icon name="plus" size={9} /> What it adds</div>
        <AddsList m={m} fresh={fresh} />
      </div>
      <div className="ext-sec">
        <div className="ext-sec-h"><Icon name="key" size={9} /> What it can do</div>
        <PermissionList perms={m.permissions ?? []} fresh={fresh} />
        {hasPanels && <p className="dim ext-fine">Panels are sandboxed. No web, no files, only the permissions above</p>}
      </div>

      {picks.length > 0 && (
        <div className="ext-sec">
          <div className="ext-sec-h"><Icon name="folder" size={9} /> Programs to find</div>
          {picks.map((t) => (
            <div key={t.id} className="ext-pick">
              <span className="m-main"><span className="m-t">{t.name}</span><span className="dim mono ext-path">{paths[t.id] ?? "Not chosen, can be set later"}</span></span>
              <Key size="sm" icon="folder" onClick={() => choose(t.id, t.pick?.title ?? `Find ${t.pick?.file ?? t.name}`)}>{paths[t.id] ? "Change" : `Choose ${t.pick?.file ?? "a program"}`}</Key>
            </div>
          ))}
        </div>
      )}

      <div className="actions" style={{ justifyContent: "flex-end" }}>
        <Key variant="ghost" onClick={onClose} disabled={busy}>{mode === "update" ? "Not now" : "Cancel"}</Key>
        <Key variant="primary" icon="check" onClick={() => void go()} busy={busy}>{mode === "install" ? "Install and turn on" : mode === "update" ? "Allow and update" : "Allow and turn on"}</Key>
      </div>
    </Dialog>
  );
}
