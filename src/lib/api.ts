// The bridge to pocket-core. Inside Tauri every call goes through `invoke`; in a plain browser (vite dev,
// tests, screenshots) the same calls go to an in-memory mock so the UI can be worked on without Windows.

import type * as T from "./types";

type Args = Record<string, unknown> | undefined;
type Listener = (payload: unknown) => void;

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

let mockMod: typeof import("./mock") | null = null;
async function mock() {
  if (!mockMod) mockMod = await import("./mock");
  return mockMod;
}

export async function call<R>(cmd: string, args?: Args): Promise<R> {
  if (inTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<R>(cmd, args);
  }
  return (await mock()).handle(cmd, args) as Promise<R>;
}

const localListeners = new Map<string, Set<Listener>>();

/** Subscribes to a core event ("queue", "installed", "error", "nxm", "collection"). Returns the unsubscribe. */
export function on(event: string, cb: Listener): () => void {
  if (inTauri) {
    let un: (() => void) | null = null;
    let cancelled = false;
    import("@tauri-apps/api/event").then(({ listen }) =>
      listen(event, (e) => cb(e.payload)).then((u) => {
        if (cancelled) u();
        else un = u;
      }),
    );
    return () => {
      cancelled = true;
      un?.();
    };
  }
  if (!localListeners.has(event)) localListeners.set(event, new Set());
  localListeners.get(event)!.add(cb);
  return () => localListeners.get(event)?.delete(cb);
}

/** Used by the mock to push events. */
export function emitLocal(event: string, payload: unknown) {
  localListeners.get(event)?.forEach((cb) => cb(payload));
}

/** Folder or file pickers: Tauri's dialog plugin, or a prompt in the browser preview. */
export async function pickFolder(title: string): Promise<string | null> {
  if (inTauri) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const r = await open({ directory: true, title });
    return typeof r === "string" ? r : null;
  }
  return window.prompt(title, "D:\\SteamLibrary\\steamapps\\common\\Cyberpunk 2077");
}

export async function pickFile(title: string, extensions: string[]): Promise<string | null> {
  if (inTauri) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const r = await open({ title, filters: [{ name: extensions.join(", "), extensions }] });
    return typeof r === "string" ? r : null;
  }
  return window.prompt(title, `C:\\Downloads\\example.${extensions[0]}`);
}

export async function pickSave(title: string, defaultPath: string): Promise<string | null> {
  if (inTauri) {
    const { save } = await import("@tauri-apps/plugin-dialog");
    return (await save({ title, defaultPath, filters: [{ name: "Collection", extensions: ["json"] }] })) ?? null;
  }
  return window.prompt(title, defaultPath);
}

export async function windowAction(a: "minimize" | "maximize" | "close") {
  if (!inTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const w = getCurrentWindow();
  if (a === "minimize") await w.minimize();
  else if (a === "maximize") await w.toggleMaximize();
  else await w.close();
}

// typed wrappers -----------------------------------------------------------------------------------------

export const api = {
  status: () => call<T.AppStatus>("status"),
  settings: () => call<T.Settings>("settings_get"),
  saveSettings: (settings: T.Settings) => call<T.Settings>("settings_save", { settings }),
  setApiKey: (key: string) => call<T.Account>("set_api_key", { key }),
  clearApiKey: () => call<void>("clear_api_key"),
  refreshAccount: () => call<T.Account>("refresh_account"),
  probe: () => call<unknown>("nexus_probe"),
  detectGames: () => call<T.GameInstall[]>("detect_games"),
  setGameRoot: (path: string) => call<T.GameInstall>("set_game_root", { path }),

  browse: (query: T.BrowseQuery) => call<T.ModPage>("browse", { query }),
  categories: () => call<T.Category[]>("categories"),
  modDetail: (modId: number) => call<T.ModDetail>("mod_detail", { modId }),
  setHidden: (modId: number, hidden: boolean) => call<void>("set_hidden", { modId, hidden }),
  endorse: (modId: number, endorse: boolean) => call<void>("endorse", { modId, endorse }),
  setTracked: (modId: number, tracked: boolean) => call<void>("set_tracked", { modId, tracked }),
  checkUpdates: () => call<number>("check_updates"),
  openNexus: (modId: number, fileId?: number | null) => call<void>("open_nexus", { modId, fileId: fileId ?? null }),

  queue: () => call<T.QueueView>("queue_view"),
  queueMod: (modId: number, fileId?: number | null) => call<number[]>("queue_mod", { modId, fileId: fileId ?? null }),
  queueUpdates: () => call<number>("queue_updates"),
  queueAction: (action: "retry" | "skip" | "remove" | "clear" | "pause" | "resume" | "open", id?: number) =>
    call<void>("queue_action", { action, id: id ?? null }),
  handleNxm: (link: string) => call<unknown>("handle_nxm", { link }),

  library: () => call<T.LibraryRow[]>("library"),
  installFile: (path: string) => call<T.InstallReport>("install_file", { path }),
  setEnabled: (key: string, enabled: boolean) => call<void>("set_enabled", { key, enabled }),
  removeMod: (key: string) => call<void>("remove_mod", { key }),
  redeploy: (key: string) => call<void>("redeploy", { key }),
  profiles: () => call<{ profiles: string[]; active: string }>("profiles"),
  createProfile: (name: string, copyCurrent: boolean) => call<void>("create_profile", { name, copyCurrent }),
  deleteProfile: (name: string) => call<void>("delete_profile", { name }),
  switchProfile: (name: string) => call<void>("switch_profile", { name }),

  loadOrder: () => call<T.LoadOrderView>("load_order"),
  setLoadOrder: (order: string[]) => call<void>("set_load_order", { order }),
  redmodDeploy: () => call<T.DeployOutcome>("redmod_deploy"),
  health: () => call<T.HealthReport>("health"),

  collections: (text: string, matchingVersion: boolean, sort: string, offset: number) =>
    call<{ collections: T.CollectionCard[]; total: number }>("collections_browse", { text, matchingVersion, sort, offset }),
  collectionFetch: (slug: string, revision: number | null, includeOptional: boolean) =>
    call<{ collection: T.Collection; check: T.PreCheck }>("collection_fetch", { slug, revision, includeOptional }),
  collectionPrecheck: (collection: T.Collection, includeOptional: boolean) => call<T.PreCheck>("collection_precheck", { collection, includeOptional }),
  collectionInstall: (collection: T.Collection, slug: string, includeOptional: boolean) =>
    call<number[]>("collection_install", { collection, slug, includeOptional }),
  collectionImport: (path: string, includeOptional: boolean) => call<{ collection: T.Collection; check: T.PreCheck }>("collection_import", { path, includeOptional }),
  collectionExport: (name: string, author: string, description: string, keys: string[], path: string | null) =>
    call<T.Collection>("collection_export", { name, author, description, keys, path }),
  savedCollections: () => call<{ name: string; collection: T.Collection }[]>("saved_collections"),
  deleteSavedCollection: (name: string) => call<void>("delete_saved_collection", { name }),
  collectionPublish: () => call<void>("collection_publish"),

  modioBrowse: (text: string, offset: number) => call<{ mods: T.ModioMod[]; total: number }>("modio_browse", { text, offset }),
  modioSetKey: (key: string) => call<void>("modio_set_key", { key }),

  nxmStatus: () => call<T.HandlerStatus>("nxm_status"),
  nxmRegister: () => call<T.HandlerStatus>("nxm_register"),
  nxmRestore: () => call<T.HandlerStatus>("nxm_restore"),
  appUpdateCheck: () => call<T.Release | null>("app_update_check"),
};
