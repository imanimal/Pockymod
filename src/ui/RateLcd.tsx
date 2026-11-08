import { useEffect, useState } from "react";
import { nexus, type RateDisplay } from "../lib/api";
import { LcdLevel } from "./pocket";

/** The rate-limit LCD: Nexus requests left. Green normal, amber low, red throttled. */
export function RateLcd() {
  const [r, setR] = useState<RateDisplay | null>(null);
  useEffect(() => {
    let off: (() => void) | undefined, gone = false;
    // an event that lands first is newer than the first reading
    nexus.rateLimit().then((x) => setR((cur) => cur ?? x)).catch(() => {});
    nexus.onRateLimit(setR).then((u) => { if (gone) u(); else off = u; });
    return () => { gone = true; off?.(); };
  }, []);
  if (!r) return null;
  const tone = r.level === "throttled" ? " bad" : r.level === "low" ? " hot" : "";
  const value = r.level === "throttled" && r.remaining === null ? "0" : r.remaining === null ? "---" : String(r.remaining);
  const sub = r.level === "throttled"
    ? `Throttled until ${time(r.throttledUntil ?? r.resetsAt)}`
    : r.level === "unknown" ? "Not reported yet" : `Requests left this ${r.window}`;
  const blocks = r.remaining !== null && r.limit ? Math.ceil((r.remaining / r.limit) * 8) : 0;
  return (
    <div className={`display sm${tone}`} role="status" aria-label="Nexus requests remaining">
      <span className="d-lbl">Nexus API</span>
      <span className="d-num">{value}{r.limit ? <small>/{r.limit}</small> : null}</span>
      <span className="d-sub">{sub}</span>
      {r.limit ? <LcdLevel on={blocks} of={8} /> : null}
    </div>
  );
}

function time(iso: string | null): string {
  if (!iso) return "the reset";
  return new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}
