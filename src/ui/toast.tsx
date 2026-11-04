import { useEffect, useState } from "react";
import { Icon } from "./Icon";

type Toast = { id: number; text: string; bad?: boolean };
let next = 1;
const subs = new Set<(t: Toast) => void>();

/** Show a short message in the corner. */
export function toast(text: string, bad = false) { const t = { id: next++, text, bad }; subs.forEach((s) => s(t)); }

export function Toasts() {
  const [list, setList] = useState<Toast[]>([]);
  useEffect(() => {
    const add = (t: Toast) => {
      setList((l) => [...l.slice(-3), t]);
      setTimeout(() => setList((l) => l.filter((x) => x.id !== t.id)), 5200);
    };
    subs.add(add);
    return () => { subs.delete(add); };
  }, []);
  return (
    <div className="toasts" role="status" aria-live="polite">
      {list.map((t) => <div key={t.id} className={`toast${t.bad ? " bad" : ""}`}><Icon name={t.bad ? "warn" : "check"} /><span>{t.text}</span></div>)}
    </div>
  );
}
