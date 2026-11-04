import { useEffect, type ReactNode } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./icons";
import { Key } from "./pocket";

/** A dialog: a module on a solid scrim. Escape or the band's close key closes it. */
export function Dialog({ icon, title, count, band, onClose, children }: { icon: IconName; title: string; count?: ReactNode; band?: "hot" | "bad"; onClose: () => void; children: ReactNode }) {
  useEffect(() => {
    const k = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onClose]);
  return (
    <div className="ov" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <section className={`panel dialog${band ? ` ${band}` : ""}`} role="dialog" aria-modal="true" aria-label={title}>
        <header className="ph">
          <span className="pt"><Icon name={icon} />{title}</span>
          {count !== undefined && <span className="cnt">{count}</span>}
          <span className="sp" />
          <Key variant="ghost" size="sm" icon="x" onClick={onClose} aria-label="Close" />
        </header>
        <div className="pb">{children}</div>
      </section>
    </div>
  );
}
