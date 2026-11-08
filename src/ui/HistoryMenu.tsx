// Undo, redo and the list of this session's mod changes, in the title bar. Removed mods are also kept for days in
// Library, Recently removed.
import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";
import { Key } from "./pocket";
import { redo, undo, useHistory } from "../lib/history";

function when(at: number): string {
  const s = (Date.now() - at) / 1000;
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  return new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export function HistoryMenu({ onRemoved }: { onRemoved: () => void }) {
  const h = useHistory();
  const [open, setOpen] = useState(false);
  // the key whose undo or redo is running (an entry's id for the list's keys); undoing a removal can take a moment
  const [working, setWorking] = useState<"undo" | "redo" | number | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const close = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOpen(false); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => { document.removeEventListener("mousedown", close); document.removeEventListener("keydown", esc); };
  }, []);
  const go = (key: "undo" | "redo" | number, job: () => Promise<boolean>) => { setWorking(key); void job().finally(() => setWorking(null)); };
  return (
    <div className="hist" ref={ref}>
      <Key size="sm" variant="ghost" icon="undo" disabled={!h.canUndo || working !== null} busy={working === "undo"} onClick={() => go("undo", () => undo())} aria-label="Undo" title="Undo (Ctrl+Z)" />
      <Key size="sm" variant="ghost" icon="redo" disabled={!h.canRedo || working !== null} busy={working === "redo"} onClick={() => go("redo", () => redo())} aria-label="Redo" title="Redo (Ctrl+Y)" />
      <Key size="sm" variant="ghost" icon="history" latched={open} onClick={() => setOpen(!open)} aria-label="History" title="History" />
      {open && (
        <div className="pmenu panel hmenu" role="dialog" aria-label="History">
          <div className="ph"><span className="pt"><Icon name="history" />History</span><span className="cnt">{h.entries.length}</span></div>
          <div className="pb">
            {h.entries.length === 0 && <p className="dim" style={{ padding: ".4em .2em" }}>No changes yet</p>}
            <div className="rows">
              {h.entries.slice(0, 30).map((e) => (
                <div key={e.id} className={`row${e.state !== "done" ? " off" : ""}`} style={{ cursor: "default" }}>
                  <span className="m-main"><span className="m-t" style={{ whiteSpace: "normal" }}>{e.label}</span><span className="m-meta"><span>{when(e.at)}</span>{e.state !== "done" && <span>undone</span>}</span></span>
                  <span className="m-side" style={{ flexDirection: "row" }}>
                    {e.state === "done" && <Key size="sm" variant="ghost" icon="undo" disabled={working !== null} busy={working === e.id} onClick={() => go(e.id, () => undo(e))}
                      aria-label={`Undo: ${e.label}`} title="Undo this and the changes after it" />}
                    {e.state === "undone" && <Key size="sm" variant="ghost" icon="redo" disabled={working !== null} busy={working === e.id} onClick={() => go(e.id, () => redo(e))}
                      aria-label={`Redo: ${e.label}`} title="Redo this and the changes before it" />}
                  </span>
                </div>
              ))}
            </div>
            <div className="actions" style={{ marginTop: ".5em" }}>
              <Key size="sm" variant="ghost" icon="trash" onClick={() => { setOpen(false); onRemoved(); }}>Recently removed</Key>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
