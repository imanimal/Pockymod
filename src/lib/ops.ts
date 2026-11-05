// Mod operations that can be undone. Screens call these instead of the raw API so every change lands in the
// history (src/lib/history.ts) with its inverse.
import { bulk, groups, library, profiles, type InstallRow } from "./api";
import { record } from "./history";
import { toast } from "../ui/toast";

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const named = (rows: Pick<InstallRow, "name">[]) => rows.length === 1 ? rows[0].name : plural(rows.length, "mod");

function failed(r: { done: number; failed: Array<[number, string]> }) {
  if (r.failed.length) toast(`${plural(r.failed.length, "mod")} could not be changed: ${r.failed[0][1]}`, true);
}

/** Turn mods on or off. Undo puts each back the way it was. */
export async function setEnabled(rows: InstallRow[], on: boolean) {
  const change = rows.filter((r) => r.enabled !== on);
  if (!change.length) return;
  const ids = change.map((r) => r.id);
  const r = await bulk(ids, on ? "enable" : "disable");
  failed(r);
  if (!r.done) return;
  record({
    label: `Turned ${on ? "on" : "off"} ${named(change)}`,
    undo: () => bulk(ids, on ? "disable" : "enable-win"),
    redo: () => bulk(ids, on ? "enable" : "disable"),
  });
}

/** Remove mods. They go to Recently removed; Undo puts them back (files, group, on or off). */
export async function remove(rows: InstallRow[]) {
  if (!rows.length) return;
  const ids = rows.map((r) => r.id);
  const r = await bulk(ids, "uninstall");
  failed(r);
  if (!r.done) return;
  const gone = new Set(ids);
  const trashIds = async () => (await library.trash()).filter((t) => gone.has(t.installId)).map((t) => t.id);
  record({
    label: `Removed ${named(rows)}`,
    undo: async () => { const t = await trashIds(); if (!t.length) throw new Error("they are no longer in Recently removed"); await library.restore(t); },
    redo: () => bulk(ids, "uninstall"),
  });
}

/** Move mods to a group (null: back to the app's own group). Undo puts each in its old group. */
export async function moveToGroup(rows: InstallRow[], group: string | null) {
  if (!rows.length) return;
  const ids = rows.map((r) => r.id);
  await groups.set(ids, group);
  const before = new Map<string | null, number[]>();
  for (const r of rows) { const g = r.customGroup ? r.group : null; before.set(g, [...(before.get(g) ?? []), r.id]); }
  record({
    label: group ? `Moved ${named(rows)} to ${group}` : `Put ${named(rows)} back in their own groups`,
    undo: async () => { for (const [g, list] of before) await groups.set(list, g); },
    redo: () => groups.set(ids, group),
  });
}

/** Switch the active profile. Undo switches back. */
export async function switchProfile(from: { id: number; name: string } | undefined, to: { id: number; name: string }) {
  const rep = await profiles.activate(to.id);
  if (!from || from.id === to.id) return rep;
  record({ label: `Switched to the ${to.name} profile`, undo: () => profiles.activate(from.id), redo: () => profiles.activate(to.id) });
  return rep;
}

/** Put removed mods back from Recently removed (not itself undoable; removing them again is). */
export async function restoreRemoved(trashIds: number[], names: string[]) {
  const ids = await library.restore(trashIds);
  const ok = ids.filter((x) => x > 0);
  toast(ok.length ? `Restored ${names.length === 1 ? names[0] : plural(ok.length, "mod")}` : "Couldn't restore", ok.length === 0);
  return ok;
}

/** Change the load order. Undo restores the old order. */
export async function setOrder(before: number[], after: number[], what: string) {
  await profiles.setOrder(after);
  record({ label: `Moved ${what}`, undo: () => profiles.setOrder(before), redo: () => profiles.setOrder(after) }, true);
}
