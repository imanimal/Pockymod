// Drop .zip, .7z or .rar files anywhere on the window to install them; a "Manual download" from the Nexus window
// is offered for install when it finishes.
import { useEffect, useState } from "react";
import { Icon } from "./Icon";
import { toast } from "./toast";
import { errorText, onManualDownload, type ChoicesNeeded, type InstallOutcome } from "../lib/api";
import { ARCHIVE, installArchive } from "../lib/archives";
import { inTauri } from "../lib/tauri";

export function DropZone({ onChoices }: { onChoices: (c: ChoicesNeeded) => void }) {
  const [over, setOver] = useState(false);
  const [busy, setBusy] = useState(0);

  const install = async (paths: string[]) => {
    const list = paths.filter((p) => ARCHIVE.test(p));
    if (!list.length) { toast("Only .zip, .7z and .rar archives can be installed.", true); return; }
    setBusy(list.length);
    for (const p of list) {
      try {
        const { outcome, request } = await installArchive(p);
        report(outcome, request.name, request, onChoices);
      } catch (e) { toast(errorText(e), true); }
      setBusy((n) => n - 1);
    }
  };

  useEffect(() => {
    if (!inTauri()) return;
    const offs: Array<() => void> = [];
    import("@tauri-apps/api/webview").then(({ getCurrentWebview }) => getCurrentWebview().onDragDropEvent((e) => {
      const t = e.payload.type;
      if (t === "enter" || t === "over") setOver(true);
      else if (t === "leave") setOver(false);
      else if (t === "drop") { setOver(false); void install(e.payload.paths); }
    })).then((u) => offs.push(u)).catch(() => {});
    onManualDownload((e) => {
      if (!e.success || !e.path) { if (!e.success) toast("The download from the Nexus window did not finish.", true); return; }
      const path = e.path;
      const name = path.split(/[\\/]/).pop() ?? "the file";
      if (!ARCHIVE.test(path)) { toast(`Downloaded ${name} to the library's downloads folder.`); return; }
      toast(`Downloaded ${name}.`, false, { label: "Install", run: () => void install([path]) });
    }).then((u) => offs.push(u));
    return () => offs.forEach((f) => f());
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!over && !busy) return null;
  return (
    <div className={`dropzone${over ? " over" : ""}`} aria-live="polite">
      <div className="dropzone-card">
        <Icon name="install" size={36} />
        <b>{over ? "Drop to install" : `Installing ${busy} ${busy === 1 ? "archive" : "archives"}`}</b>
        <span className="dim">{over ? ".zip, .7z or .rar. Files named like Nexus downloads are matched to their mod for updates." : "This takes a moment for big archives."}</span>
      </div>
    </div>
  );
}

function report(o: InstallOutcome, name: string, request: ChoicesNeeded["request"], onChoices: (c: ChoicesNeeded) => void) {
  switch (o.status) {
    case "installed": toast(`Installed ${name}.`); if (o.needsRedmodDeploy) toast("This is a REDmod: run the REDmod deploy from Load order."); return;
    case "needsChoices": onChoices({ request, fomod: o.fomod, defaults: o.defaults }); return;
    case "needsReview": toast(`${name} has files with no known place. Finish it in Library, Downloaded.`, true); return;
    case "staged": toast(`${name} is in, but clashes with other mods. Decide in Library.`, true); return;
    case "alreadyHave": toast(`You already have ${name}.`); return;
  }
}
