// The one place the UI touches Tauri. Outside Tauri (plain `npm run dev` in a browser) calls go to the
// in-memory demo backend instead, so the UI can be worked on and tested without the Rust side.
import type { InvokeArgs } from "@tauri-apps/api/core";

export const inTauri = (): boolean => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Handler = (args: Record<string, unknown>) => unknown | Promise<unknown>;
const demo: Record<string, Handler> = {};

/** Register demo handlers used when running outside Tauri. */
export function registerDemo(handlers: Record<string, Handler>) { Object.assign(demo, handlers); }

export async function call<T>(cmd: string, args: InvokeArgs = {}): Promise<T> {
  if (inTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  const h = demo[cmd];
  if (!h) throw new Error(`No demo handler for ${cmd}`);
  return (await h(args as Record<string, unknown>)) as T;
}

type Unlisten = () => void;
const demoListeners = new Map<string, Set<(p: unknown) => void>>();

export async function listen<T>(event: string, cb: (payload: T) => void): Promise<Unlisten> {
  if (inTauri()) {
    const { listen: l } = await import("@tauri-apps/api/event");
    return l<T>(event, (e) => cb(e.payload));
  }
  // each listen gets its own entry, like Tauri's, so unlistening one never removes another with the same callback
  const set = demoListeners.get(event) ?? new Set();
  const h = (p: unknown) => cb(p as T);
  set.add(h);
  demoListeners.set(event, set);
  return () => set.delete(h);
}

/** A listener's unlisten, usable before the listener is in place: called early, it removes the listener as soon as
 *  it arrives (an effect cleaned up before `listen` resolved would otherwise leave it running). */
export function held(p: Promise<Unlisten>): Unlisten {
  let gone = false, off: Unlisten | undefined;
  p.then((u) => { if (gone) u(); else off = u; }).catch(() => {});
  return () => { gone = true; off?.(); off = undefined; };
}

/** Demo backend: emit an event to listeners. */
export function demoEmit(event: string, payload: unknown) {
  demoListeners.get(event)?.forEach((cb) => cb(payload));
}

export async function windowAction(action: "minimize" | "toggleMaximize" | "close" | "startDragging") {
  if (!inTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const w = getCurrentWindow();
  await w[action]();
}
