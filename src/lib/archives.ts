// Installing an archive that did not come through the queue: Install from file, a file dropped on the window, or
// a manual download from the Nexus window. The archive is matched to its Nexus mod when its name allows, so it gets
// update checks like any other.
import { identifyArchive, library, type InstallOutcome } from "./api";

export const ARCHIVE = /\.(zip|7z|rar)$/i;

/** A stable made-up id for an archive that is not a known Nexus file (900 000 000 and up are local). */
function localId(name: string): number { return 900_000_000 + (Array.from(name).reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7) % 90_000_000); }

export async function installArchive(archive: string): Promise<{ outcome: InstallOutcome; request: { modId: number; fileId: number; name: string; version: string; archive: string } }> {
  const id = await identifyArchive(archive).catch(() => null);
  const base = archive.split(/[\\/]/).pop()?.replace(ARCHIVE, "") ?? "Mod";
  const name = id?.name || base;
  const modId = id?.modId ?? localId(base);
  const fileId = id?.fileId ?? localId(`${base}#file`);
  const request = { modId, fileId, name, version: id?.version || (id?.modId ? "" : "local"), archive };
  const outcome = await library.installFile({ ...request, fileName: id?.fileName ?? null });
  return { outcome, request };
}
