// App-wide state: the status (account, game, rate limit, queue summary), navigation, and toasts.

import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { api, on } from "./api";
import { errMsg } from "./format";
import type { AppStatus, InstallReport } from "./types";

export type View = "browse" | "library" | "collections" | "downloads" | "loadorder" | "health" | "settings" | "gallery";

export interface Toast {
  id: number;
  message: string;
  tone?: "bad";
  action?: { label: string; run: () => void };
}

interface Ctx {
  status: AppStatus | null;
  refresh: () => void;
  view: View;
  go: (v: View, focus?: unknown) => void;
  focus: unknown;
  toast: (message: string, opts?: { tone?: "bad"; action?: Toast["action"] }) => void;
  fail: (e: unknown) => void;
  toasts: Toast[];
  dismiss: (id: number) => void;
  /** Bumped when the library changes, so screens refetch. */
  libraryRev: number;
  healthProblems: number;
}

const AppCtx = createContext<Ctx | null>(null);

export function useApp(): Ctx {
  const c = useContext(AppCtx);
  if (!c) throw new Error("useApp outside AppProvider");
  return c;
}

export function AppProvider({ children, initialView = "browse" }: { children: ReactNode; initialView?: View }) {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [view, setView] = useState<View>(initialView);
  const [focus, setFocus] = useState<unknown>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [libraryRev, setLibraryRev] = useState(0);
  const [healthProblems, setHealthProblems] = useState(0);
  const nextId = useRef(1);

  const refresh = useCallback(() => {
    api.status().then(setStatus).catch(() => {});
    api.health().then((h) => setHealthProblems(h.problems)).catch(() => {});
  }, []);

  const toast = useCallback((message: string, opts?: { tone?: "bad"; action?: Toast["action"] }) => {
    const id = nextId.current++;
    setToasts((t) => [...t.slice(-3), { id, message, ...opts }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), opts?.tone === "bad" ? 9000 : 5000);
  }, []);

  const fail = useCallback((e: unknown) => toast(errMsg(e), { tone: "bad" }), [toast]);

  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 5000);
    const offs = [
      on("queue", () => api.status().then(setStatus).catch(() => {})),
      on("installed", (p) => {
        const r = p as InstallReport;
        const extra = r.conflicts?.length ? `, ${r.conflicts.length} file conflict(s)` : "";
        toast(`Installed ${r.name}${extra}.`);
        setLibraryRev((n) => n + 1);
        refresh();
      }),
      on("error", (p) => toast(errMsg(p), { tone: "bad" })),
      on("toast", (p) => toast(errMsg(p))),
      on("collection", (p) => {
        setFocus(p);
        setView("collections");
      }),
    ];
    return () => {
      clearInterval(t);
      offs.forEach((o) => o());
    };
  }, [refresh, toast]);

  const go = useCallback((v: View, f?: unknown) => {
    setFocus(f ?? null);
    setView(v);
  }, []);

  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), []);

  const value: Ctx = {
    status,
    refresh: () => {
      refresh();
      setLibraryRev((n) => n + 1);
    },
    view,
    go,
    focus,
    toast,
    fail,
    toasts,
    dismiss,
    libraryRev,
    healthProblems,
  };
  return <AppCtx.Provider value={value}>{children}</AppCtx.Provider>;
}

/** Runs an async action with busy state and error toasts. */
export function useAction() {
  const { fail } = useApp();
  const [busy, setBusy] = useState<string | null>(null);
  const run = useCallback(
    async <T,>(name: string, f: () => Promise<T>): Promise<T | undefined> => {
      setBusy(name);
      try {
        return await f();
      } catch (e) {
        fail(e);
        return undefined;
      } finally {
        setBusy(null);
      }
    },
    [fail],
  );
  return { busy, run };
}

/** Theme: follows the system until the person picks Cream or Night. */
export function applyTheme(theme: "system" | "cream" | "night") {
  const root = document.documentElement;
  const dark = theme === "night" || (theme === "system" && window.matchMedia?.("(prefers-color-scheme: dark)").matches);
  root.dataset.scheme = dark ? "dark" : "light";
}
