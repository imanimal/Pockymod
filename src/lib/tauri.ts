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
  const set = demoListeners.get(event) ?? new Set();
  set.add(cb as (p: unknown) => void);
  demoListeners.set(event, set);
  return () => set.delete(cb as (p: unknown) => void);
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
