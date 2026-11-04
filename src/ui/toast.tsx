// Toasts: short messages at the bottom of the window. One can carry an action (Undo, Redo, Open); an Undo toast
// turns into a Redo toast once used, like mail apps do.
import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";

export interface ToastAction { label: string; run: () => void | Promise<void> }
type Toast = { id: number; text: string; bad?: boolean; action?: ToastAction; ms: number; leaving?: boolean };
let next = 1;
const subs = new Set<(t: Toast) => void>();
const updates = new Set<(id: number, patch: Partial<Toast>) => void>();

/** Show a short message. With an action, it stays longer and offers one key. Returns its id. */
export function toast(text: string, bad = false, action?: ToastAction): number {
  const t: Toast = { id: next++, text, bad, action, ms: action ? 9000 : bad ? 7000 : 4600 };
  subs.forEach((s) => s(t));
  return t.id;
}
/** Change a toast in place (an Undo that became a Redo) and restart its timer. */
export function retoast(id: number, text: string, action?: ToastAction, bad = false) { updates.forEach((u) => u(id, { text, action, bad, ms: action ? 9000 : 4600 })); }

export function Toasts() {
  const [list, setList] = useState<Toast[]>([]);
  const timers = useRef(new Map<number, number>());
  useEffect(() => {
    const arm = (t: Toast) => {
      const old = timers.current.get(t.id);
      if (old) clearTimeout(old);
      timers.current.set(t.id, window.setTimeout(() => leave(t.id), t.ms));
    };
    const leave = (id: number) => {
      setList((l) => l.map((x) => x.id === id ? { ...x, leaving: true } : x));
      window.setTimeout(() => setList((l) => l.filter((x) => x.id !== id)), 220);
      timers.current.delete(id);
    };
    const add = (t: Toast) => { setList((l) => [...l.filter((x) => !x.leaving).slice(-3), t]); arm(t); };
    const patch = (id: number, p: Partial<Toast>) => setList((l) => l.map((x) => { if (x.id !== id) return x; const n = { ...x, ...p, leaving: false }; arm(n); return n; }));
    subs.add(add); updates.add(patch);
    return () => { subs.delete(add); updates.delete(patch); };
  }, []);
  const hold = (id: number, on: boolean) => {
    const t = list.find((x) => x.id === id);
    if (!t) return;
    const old = timers.current.get(id);
    if (old) clearTimeout(old);
    if (!on) timers.current.set(id, window.setTimeout(() => {
      setList((l) => l.map((x) => x.id === id ? { ...x, leaving: true } : x));
      window.setTimeout(() => setList((l) => l.filter((x) => x.id !== id)), 220);
    }, 2400));
  };
  return (
    <div className="toasts" role="status" aria-live="polite">
      {list.map((t) => (
        <div key={`${t.id}-${t.text}`} className={`toast${t.bad ? " bad" : ""}${t.action ? " act" : ""}${t.leaving ? " leaving" : ""}`}
          onMouseEnter={() => hold(t.id, true)} onMouseLeave={() => hold(t.id, false)}>
          <Icon name={t.bad ? "warn" : t.action?.label === "Redo" ? "redo" : "check"} />
          <span className="tt">{t.text}</span>
          {t.action && <button type="button" className="toast-act" onClick={() => void t.action!.run()}>{t.action.label}</button>}
          <button type="button" className="toast-x" aria-label="Dismiss" onClick={() => setList((l) => l.filter((x) => x.id !== t.id))}><Icon name="x" size={9} /></button>
          <i className="toast-time" style={{ animationDuration: `calc(var(--m) * ${t.ms}ms + 1ms)` }} />
        </div>
      ))}
    </div>
  );
}
