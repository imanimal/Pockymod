// Parts the Create tabs share: the folder or archive being checked and packed, the hand-off to the publish form,
// and small helpers for copying and opening Nexus pages.
import { useState } from "react";
import { Pill } from "../../ui/pocket";
import { creator, errorText, nexusPages, pickArchive, pickFolder, type CreatorCheck } from "../../lib/api";
import { toast } from "../../ui/toast";

export type FileCategory = "main" | "optional" | "miscellaneous";

/** What "New version" on a file hands to the publish form. */
export interface PublishPrefill { modId: number; modName: string; fileId: string; fileName: string; category: FileCategory; currentVersion: string }

export const categoryOf = (c: string): FileCategory => c === "OPTIONAL" ? "optional" : c === "MISCELLANEOUS" ? "miscellaneous" : "main";
export const fileCategoryText = (c: string) => c.toLowerCase().replace(/_/g, " ");
export const unixDate = (s: number) => new Date(s * 1000).toISOString();

export interface PackState { path: string | null; isFolder: boolean; name: string; version: string; check: CreatorCheck | null; zip: string | null; busy: boolean }
export type Pack = PackState & {
  set: (p: Partial<PackState>) => void;
  pick: (folder: boolean) => Promise<void>;
  pack: () => Promise<void>;
};

/** The mod being prepared: picked folder or archive, its check, and the clean zip once packed. */
export function usePack(): Pack {
  const [s, setS] = useState<PackState>({ path: null, isFolder: false, name: "", version: "1.0.0", check: null, zip: null, busy: false });
  const set = (p: Partial<PackState>) => setS((x) => ({ ...x, ...p }));
  const pick = async (folder: boolean) => {
    const p = folder ? await pickFolder("Your mod's folder") : await pickArchive();
    if (!p) return;
    const guess = p.split(/[\\/]/).pop()?.replace(/\.(zip|7z|rar)$/i, "") ?? "My mod";
    const name = s.name || guess;
    set({ path: p, isFolder: folder, zip: folder ? null : p, name, check: null, busy: true });
    try { set({ check: await creator.check(p, name), busy: false }); } catch (e) { set({ busy: false }); toast(errorText(e), true); }
  };
  const pack = async () => {
    if (!s.path) return;
    set({ busy: true });
    try { const z = await creator.pack(s.path, s.name, s.version); set({ zip: z, busy: false }); toast("Packed"); } catch (e) { set({ busy: false }); toast(errorText(e), true); }
  };
  return { ...s, set, pick, pack };
}

export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

export function copy(text: string, what = "Copied") {
  navigator.clipboard.writeText(text).then(() => toast(what)).catch(() => toast("Couldn't copy", true));
}

/** A Nexus page in the app's own Nexus window. */
export function openNexus(url: string) { nexusPages.url(url).catch((e) => toast(errorText(e), true)); }

const STATUS: Record<string, [tone: "ok" | "soon" | "hot" | "over" | undefined, label: string]> = {
  published: ["ok", "Published"], hidden: ["soon", "Hidden"], under_moderation: ["hot", "Under moderation"], not_published: [undefined, "Not published"],
  removed: ["over", "Removed"], removed_by_staff: ["over", "Removed by staff"], wastebinned: ["over", "Deleted"],
};
export function StatusPill({ status }: { status: string }) {
  const [tone, label] = STATUS[status.toLowerCase()] ?? [undefined, status.replace(/_/g, " ") || "Status unknown"];
  return <Pill tone={tone}>{label}</Pill>;
}

/** The virus scan, shown only when it is not a plain pass. */
export function ScanPill({ scan }: { scan: string | null }) {
  if (!scan || ["VERIFIED", "INTERNALLY_VERIFIED", "MANUALLY_VERIFIED"].includes(scan)) return null;
  const bad = ["QUARANTINED", "REPORT_ERROR", "FILE_NOT_FOUND"].includes(scan);
  return <Pill tone={bad ? "over" : "soon"}>{bad ? "Scan problem" : scan === "QUEUED" || scan === "WAITING_REPORT" ? "Being scanned" : "Not scanned"}</Pill>;
}
