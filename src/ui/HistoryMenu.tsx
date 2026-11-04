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
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const close = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOpen(false); };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, []);
  return (
    <div className="hist" ref={ref}>
      <Key size="sm" variant="ghost" icon="undo" disabled={!h.canUndo} onClick={() => void undo()} aria-label="Undo" title="Undo (Ctrl+Z)" />
      <Key size="sm" variant="ghost" icon="redo" disabled={!h.canRedo} onClick={() => void redo()} aria-label="Redo" title="Redo (Ctrl+Y)" />
      <Key size="sm" variant="ghost" icon="history" latched={open} onClick={() => setOpen(!open)} aria-label="History" title="What changed" />
      {open && (
        <div className="pmenu panel hmenu" role="dialog" aria-label="History">
          <div className="ph"><span className="pt"><Icon name="history" />History</span><span className="cnt">{h.entries.length}</span></div>
          <div className="pb">
            {h.entries.length === 0 && <p className="dim" style={{ padding: ".4em .2em" }}>Changes to your mods show here. Ctrl+Z undoes the last one, Ctrl+Y redoes it.</p>}
            <div className="rows">
              {h.entries.slice(0, 30).map((e) => (
                <div key={e.id} className={`row${e.state !== "done" ? " off" : ""}`} style={{ cursor: "default" }}>
                  <span className="m-main"><span className="m-t" style={{ whiteSpace: "normal" }}>{e.label}</span><span className="m-meta"><span>{when(e.at)}</span>{e.state !== "done" && <span>undone</span>}</span></span>
                  <span className="m-side" style={{ flexDirection: "row" }}>
                    {e.state === "done" && <Key size="sm" variant="ghost" icon="undo" onClick={() => void undo(e)} aria-label={`Undo: ${e.label}`} />}
                    {e.state === "undone" && <Key size="sm" variant="ghost" icon="redo" onClick={() => void redo(e)} aria-label={`Redo: ${e.label}`} />}
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
