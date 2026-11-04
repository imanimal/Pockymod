// Undo and redo for mod operations (turn on/off, remove, move to a group, load order, profile switch).
// Every operation that changes the library goes through `record`, which shows a toast with Undo; using it runs the
// inverse and turns the toast into Redo. Ctrl+Z undoes, Ctrl+Y and Ctrl+Shift+Z redo (not while typing in a field,
// where the keys keep their usual meaning). The list lives for the session; removed mods also stay in Recently
// removed (the trash) for days, so a removal can be put back after a restart too.
import { useEffect, useState } from "react";
import { retoast, toast } from "../ui/toast";

export interface Op {
  /** Past tense, sentence case: "Turned off 3 mods". */
  label: string;
  undo: () => Promise<unknown>;
  redo: () => Promise<unknown>;
  at: number;
}
/** "gone": undone, then a new change was made, so it can no longer be redone. */
export interface Entry extends Op { id: number; state: "done" | "undone" | "gone" }

let seq = 1;
const past: Entry[] = [];
const future: Entry[] = [];
const log: Entry[] = []; // everything this session, newest first, for the History panel
const subs = new Set<() => void>();
let running = false;
const changed = () => subs.forEach((f) => f());

function errText(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}

/** Record a finished operation and offer Undo. */
export function record(op: Omit<Op, "at">, quiet = false): Entry {
  const e: Entry = { ...op, at: Date.now(), id: seq++, state: "done" };
  past.push(e);
  if (past.length > 100) past.shift();
  // a new change ends the redo chain: those entries stay in the list, marked gone
  for (const f of future) f.state = "gone";
  future.length = 0;
  log.unshift(e);
  if (log.length > 200) log.pop();
  changed();
  if (!quiet) offerUndo(e);
  return e;
}

// each entry's toast, so undoing from the keyboard or the title bar turns that same toast into Redo
const toastOf = new Map<number, number>();
function show(e: Entry, text: string, label: "Undo" | "Redo", toastId?: number) {
  let id = toastId ?? toastOf.get(e.id) ?? 0;
  const action = { label, run: () => void (label === "Undo" ? undo(e, id) : redo(e, id)) };
  if (!id || !retoast(id, text, action)) id = toast(text, false, action);
  toastOf.set(e.id, id);
}
function offerUndo(e: Entry, toastId?: number) { show(e, `${e.label}.`, "Undo", toastId); }
function offerRedo(e: Entry, toastId?: number) { show(e, `Undone: ${e.label.charAt(0).toLowerCase()}${e.label.slice(1)}.`, "Redo", toastId); }

/** Undo the newest operation, or the one given (it must be the newest still done). */
export async function undo(target?: Entry, toastId?: number): Promise<boolean> {
  if (running) return false;
  const e = target ?? past[past.length - 1];
  if (!e || e.state !== "done" || !past.includes(e)) { if (!target) toast("Nothing to undo."); return false; }
  // undoing an older one first undoes what came after it, so the inverse runs on the state it expects
  while (past.length && past[past.length - 1] !== e) { const later = past[past.length - 1]; if (!(await undoOne(later))) return false; }
  const ok = await undoOne(e);
  if (ok) offerRedo(e, toastId);
  return ok;
}
async function undoOne(e: Entry): Promise<boolean> {
  running = true;
  try {
    await e.undo();
    past.splice(past.indexOf(e), 1);
    e.state = "undone";
    future.push(e);
    return true;
  } catch (err) { toast(`Could not undo: ${errText(err)}`, true); return false; }
  finally { running = false; changed(); }
}

export async function redo(target?: Entry, toastId?: number): Promise<boolean> {
  if (running) return false;
  const e = target ?? future[future.length - 1];
  if (!e || e.state !== "undone" || !future.includes(e)) { if (!target) toast("Nothing to redo."); return false; }
  while (future.length && future[future.length - 1] !== e) { const later = future[future.length - 1]; if (!(await redoOne(later))) return false; }
  const ok = await redoOne(e);
  if (ok) offerUndo(e, toastId);
  return ok;
}
async function redoOne(e: Entry): Promise<boolean> {
  running = true;
  try {
    await e.redo();
    future.splice(future.indexOf(e), 1);
    e.state = "done";
    past.push(e);
    return true;
  } catch (err) { toast(`Could not redo: ${errText(err)}`, true); return false; }
  finally { running = false; changed(); }
}

export const canUndo = () => past.length > 0;
export const canRedo = () => future.length > 0;
export const entries = () => log.slice();

/** Re-render on history changes. */
export function useHistory(): { entries: Entry[]; canUndo: boolean; canRedo: boolean } {
  const [, tick] = useState(0);
  useEffect(() => { const f = () => tick((n) => n + 1); subs.add(f); return () => { subs.delete(f); }; }, []);
  return { entries: log.slice(), canUndo: past.length > 0, canRedo: future.length > 0 };
}

/** Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z, everywhere but text fields. */
export function installUndoKeys(): () => void {
  const h = (ev: KeyboardEvent) => {
    if (!(ev.ctrlKey || ev.metaKey) || ev.altKey) return;
    const t = ev.target as HTMLElement | null;
    if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName)) && (t as HTMLInputElement).type !== "checkbox") return;
    const k = ev.key.toLowerCase();
    if (k === "z" && !ev.shiftKey) { ev.preventDefault(); void undo(); }
    else if (k === "y" || (k === "z" && ev.shiftKey)) { ev.preventDefault(); void redo(); }
  };
  window.addEventListener("keydown", h);
  return () => window.removeEventListener("keydown", h);
}
