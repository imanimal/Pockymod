// A menu that floats by the key that opened it, above everything in the window: it is placed in the shell itself,
// so a card's rounded corners, a drawer's scrolling or a module's band never clip it. It opens below the key, or
// above when there is more room there, stays inside the window, and closes on a click outside or Escape.
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

export function Popover({ anchor, onClose, children, width = "21em", label }: {
  anchor: HTMLElement | null; onClose: () => void; children: ReactNode; width?: string; label?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number; maxHeight: number } | null>(null);
  useLayoutEffect(() => {
    const place = () => {
      const el = ref.current;
      if (!anchor || !el || !anchor.isConnected) return;
      const a = anchor.getBoundingClientRect(), vw = window.innerWidth, vh = window.innerHeight, pad = 8;
      const w = el.offsetWidth, h = el.scrollHeight;
      const left = Math.max(pad, Math.min(a.right - w, vw - w - pad));
      const below = vh - a.bottom - pad - 6, above = a.top - pad - 6;
      const down = below >= Math.min(h, 360) || below >= above;
      const maxHeight = Math.max(160, down ? below : above);
      const top = down ? a.bottom + 6 : Math.max(pad, a.top - 6 - Math.min(h, maxHeight));
      setPos((p) => p && p.left === left && p.top === top && p.maxHeight === maxHeight ? p : { left, top, maxHeight });
    };
    place();
    const ro = new ResizeObserver(place);
    if (ref.current) ro.observe(ref.current);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => { ro.disconnect(); window.removeEventListener("resize", place); window.removeEventListener("scroll", place, true); };
  }, [anchor]);
  useEffect(() => {
    const down = (e: MouseEvent) => { const t = e.target as Node; if (!ref.current?.contains(t) && !anchor?.contains(t)) onClose(); };
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopPropagation(); onClose(); } };
    document.addEventListener("mousedown", down, true);
    window.addEventListener("keydown", key, true);
    return () => { document.removeEventListener("mousedown", down, true); window.removeEventListener("keydown", key, true); };
  }, [anchor, onClose]);
  const host = document.querySelector(".shell") ?? document.body;
  return createPortal(
    <div ref={ref} className="pmenu panel popover" role="menu" aria-label={label}
      style={{ width, left: pos?.left ?? -9999, top: pos?.top ?? 0, maxHeight: pos?.maxHeight, visibility: pos ? "visible" : "hidden" }}
      onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
      {children}
    </div>, host);
}
