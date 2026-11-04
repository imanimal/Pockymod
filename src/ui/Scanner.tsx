// A busy readout for anything that takes more than a moment: what is being looked at now, what was looked at,
// how far along it is, and for how long. Lines print in like a terminal; the bar fills a segment at a time.
import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./icons";

export interface ScanLine { text: string; done?: boolean }

/** Long paths keep their end, where the part that changes is. */
export function tail(text: string, max = 72): string { return text.length > max ? `…${text.slice(-(max - 1))}` : text; }

export function Scanner({ title, icon = "refresh", lines, done, total, sub }: {
  title: string; icon?: IconName; lines: ScanLine[]; done?: number; total?: number; sub?: string;
}) {
  const [secs, setSecs] = useState(0);
  const start = useRef(Date.now());
  useEffect(() => { const t = setInterval(() => setSecs(Math.floor((Date.now() - start.current) / 1000)), 500); return () => clearInterval(t); }, []);
  const frac = total ? Math.min(1, (done ?? 0) / total) : null;
  const shown = lines.slice(-6);
  return (
    <div className="scanner" role="status" aria-live="polite" aria-busy="true">
      <div className="scan-head">
        <span className="scan-eye" aria-hidden="true"><Icon name={icon} /></span>
        <span className="scan-t">{title}</span>
        <span className="display sm inline scan-clock"><span className="d-lbl">Time</span><span className="d-num">{String(Math.floor(secs / 60)).padStart(2, "0")}:{String(secs % 60).padStart(2, "0")}</span></span>
      </div>
      <div className="scan-log">
        {shown.map((l, i) => (
          <div key={`${lines.length - shown.length + i}-${l.text}`} className={`scan-ln${i === shown.length - 1 && !l.done ? " now" : ""}${l.done ? " ok" : ""}`}>
            <span className="scan-mk">{l.done ? "ok" : i === shown.length - 1 ? ">" : "·"}</span><span className="scan-tx">{tail(l.text)}</span>
          </div>
        ))}
        {shown.length === 0 && <div className="scan-ln now"><span className="scan-mk">&gt;</span><span className="scan-tx">Starting</span></div>}
      </div>
      <div className={`scan-bar${frac === null ? " sweep" : ""}`} aria-hidden="true">
        {frac === null ? <i /> : Array.from({ length: 24 }, (_, k) => <b key={k} className={k < Math.round(frac * 24) ? "on" : ""} />)}
      </div>
      {(sub || frac !== null) && <div className="scan-sub dim">{frac !== null ? `${done} of ${total}` : ""}{sub ? `${frac !== null ? " · " : ""}${sub}` : ""}</div>}
    </div>
  );
}
