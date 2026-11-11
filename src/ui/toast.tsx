// Toasts: short messages at the bottom of the window. One can carry an action (Undo, Redo, Open); an Undo toast
// turns into a Redo toast once used, like mail apps do.
import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";
import { goSettings, settingsTarget } from "../lib/nav";

export interface ToastAction { label: string; run: () => void | Promise<void> }
type Toast = { id: number; text: string; bad?: boolean; action?: ToastAction; ms: number; leaving?: boolean };
let next = 1;
const subs = new Set<(t: Toast) => void>();
const updates = new Set<(id: number, patch: Partial<Toast>) => void>();
const live = new Set<number>();

/** Show a short message. With an action, it stays longer and offers one key. A message that sends the user to Settings
 *  ("in Settings, Appearance") offers an Open Settings link that goes there. Returns its id. */
export function toast(text: string, bad = false, action?: ToastAction): number {
  const to = action ? null : settingsTarget(text);
  if (to) action = { label: "Open Settings", run: () => goSettings(to.section) };
  const t: Toast = { id: next++, text, bad, action, ms: action ? 9000 : bad ? 7000 : 4600 };
  subs.forEach((s) => s(t));
  return t.id;
}
/** Change a toast in place (an Undo that became a Redo) and restart its timer. False when it has gone. */
export function retoast(id: number, text: string, action?: ToastAction, bad = false): boolean {
  if (!live.has(id)) return false;
  updates.forEach((u) => u(id, { text, action, bad, ms: action ? 9000 : 4600 }));
  return true;
}

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
      live.delete(id);
      setList((l) => l.map((x) => x.id === id ? { ...x, leaving: true } : x));
      window.setTimeout(() => setList((l) => l.filter((x) => x.id !== id)), 220);
      timers.current.delete(id);
    };
    const add = (t: Toast) => {
      live.add(t.id);
      setList((l) => { const keep = [...l.filter((x) => !x.leaving).slice(-3), t]; for (const x of l) if (!keep.includes(x)) live.delete(x.id); return keep; });
      arm(t);
    };
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
      live.delete(id);
      setList((l) => l.map((x) => x.id === id ? { ...x, leaving: true } : x));
      window.setTimeout(() => setList((l) => l.filter((x) => x.id !== id)), 220);
    }, 2400));
  };
  return (
    <div className="toasts" role="status" aria-live="polite">
      {list.map((t) => (
        <div key={t.id} className={`toast${t.bad ? " bad" : ""}${t.action ? " act" : ""}${t.leaving ? " gone" : ""}`}
          onMouseEnter={() => hold(t.id, true)} onMouseLeave={() => hold(t.id, false)}>
          <Icon name={t.bad ? "warn" : t.action?.label === "Redo" ? "redo" : "check"} />
          <span className="tt" key={t.text}>{t.text}</span>
          {t.action && <button type="button" className="toast-act" onClick={() => void t.action!.run()}>{t.action.label}</button>}
          <button type="button" className="toast-x" aria-label="Dismiss" title="Dismiss" onClick={() => { live.delete(t.id); setList((l) => l.filter((x) => x.id !== t.id)); }}><Icon name="x" size={9} /></button>
          <i key={`${t.text}-${t.ms}`} className="t-life" style={{ animationDuration: `${t.ms}ms` }} aria-hidden="true" />
        </div>
      ))}
    </div>
  );
}
