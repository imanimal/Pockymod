import { useCallback, useEffect, useRef, type ReactNode } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./icons";
import { Key } from "./pocket";
import { leave } from "./motion";

/** A dialog: a module on a solid scrim. Escape or the band's close key closes it. `locked` says why it can't close
 *  yet (work it started is still running): the close key is off with that as its tip, and Escape and the scrim do
 *  nothing. */
export function Dialog({ icon, title, count, band, onClose, children, className, locked }: { icon: IconName; title: string; count?: ReactNode; band?: "hot" | "bad"; onClose: () => void; children: ReactNode; className?: string; locked?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  // the dialog drops back out before it closes
  const close = useCallback(() => { if (!locked) leave(ref.current, onClose); }, [onClose, locked]);
  useEffect(() => {
    const k = (e: KeyboardEvent) => { if (e.key === "Escape") close(); };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [close]);
  return (
    <div ref={ref} className="ov" onMouseDown={(e) => { if (e.target === e.currentTarget) close(); }}>
      <section className={`panel dialog${band ? ` ${band}` : ""}${className ? ` ${className}` : ""}`} role="dialog" aria-modal="true" aria-label={title}>
        <header className="ph">
          <span className="pt"><Icon name={icon} />{title}</span>
          {count !== undefined && <span className="cnt">{count}</span>}
          <span className="sp" />
          <Key variant="ghost" size="sm" icon="x" onClick={close} disabled={!!locked} aria-label="Close" title={locked ?? "Close (Esc)"} />
        </header>
        <div className="pb">{children}</div>
      </section>
    </div>
  );
}
