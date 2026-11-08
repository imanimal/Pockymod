// Endorsing, without the hassle: a quiet strip in Library when mods you have are not endorsed yet (Endorse all, Pick,
// Not now), a pick list with "Not for me", and one gentle line after you have played. Nothing here interrupts:
// no dialog opens by itself, and Not now keeps it away for two weeks.
import { useCallback, useEffect, useState } from "react";
import { Dialog } from "./Dialog";
import { Key, Switch, useBusy } from "./pocket";
import { toast } from "./toast";
import { endorse, errorText, library, type EndorseCandidate } from "../lib/api";
import { held } from "../lib/tauri";

const SNOOZE = "pm.endorse.snooze", ASKED = "pm.endorse.afterPlay", PLAYED = "pm.lastPlay";
const read = (k: string) => { try { return Number(localStorage.getItem(k)) || 0; } catch { return 0; } };
const write = (k: string, v: number) => { try { localStorage.setItem(k, String(v)); } catch { /* storage unavailable */ } };

export async function endorseAll(list: EndorseCandidate[]) {
  if (!list.length) return;
  try {
    const [ok, why] = await endorse.many(list);
    toast(ok ? `Endorsed ${ok} ${ok === 1 ? "mod" : "mods"}${why ? `. Some failed: ${why}` : ""}` : `Nothing endorsed${why ? `. ${why}` : ""}`, !ok);
  } catch (e) { toast(errorText(e), true); }
}

/** The mods not endorsed yet, kept current. */
export function useCandidates() {
  const [list, setList] = useState<EndorseCandidate[] | null>(null);
  const [hasKey, setHasKey] = useState(false);
  const load = useCallback(() => { endorse.status().then((s) => { setList(s.candidates); setHasKey(s.hasKey); }).catch(() => setList([])); }, []);
  useEffect(() => {
    load();
    window.addEventListener("pm-endorsed", load);
    const off = held(library.onChanged(load));
    return () => { window.removeEventListener("pm-endorsed", load); off(); };
  }, [load]);
  return { list: list ?? [], hasKey, ready: list !== null, reload: load };
}

/** Library's strip. */
export function EndorseStrip() {
  const { list, hasKey, ready } = useCandidates();
  const [picking, setPicking] = useState(false);
  const [busy, run] = useBusy();
  const [snoozed, setSnoozed] = useState(() => read(SNOOZE) > Date.now());
  if (!ready || !hasKey || list.length === 0 || snoozed) return picking ? <Pick list={list} onClose={() => setPicking(false)} /> : null;
  return (
    <div className="endorse-strip" role="note">
      <span className="endorse-star" aria-hidden="true">★</span>
      <span className="m-main"><b className="hi">{list.length} {list.length === 1 ? "mod" : "mods"} not endorsed</b></span>
      <Key size="sm" icon="star" busy={busy} onClick={() => run(() => endorseAll(list))}
        title={`Endorse all ${list.length} on Nexus`}>Endorse all {list.length}</Key>
      <Key size="sm" variant="ghost" onClick={() => setPicking(true)} title="Choose which ones">Pick</Key>
      <Key size="sm" variant="ghost" onClick={() => { write(SNOOZE, Date.now() + 14 * 86400_000); setSnoozed(true); }} title="Hide for two weeks">Not now</Key>
      {picking && <Pick list={list} onClose={() => setPicking(false)} />}
    </div>
  );
}

/** Choose which mods to endorse; "Not for me" stops asking about one. */
function Pick({ list, onClose }: { list: EndorseCandidate[]; onClose: () => void }) {
  const [on, setOn] = useState<Set<number>>(() => new Set(list.map((m) => m.modId)));
  const [busy, run] = useBusy();
  const chosen = list.filter((m) => on.has(m.modId));
  const decline = (m: EndorseCandidate) => endorse.decline([m.modId]).then(() => toast(`Won't ask about ${m.name} again`)).catch((e) => toast(errorText(e), true));
  return (
    <Dialog icon="star" title="Endorse your mods" count={`${chosen.length} of ${list.length}`} onClose={onClose}>
      <div className="rows" style={{ maxHeight: "22em", overflow: "auto" }}>
        {list.map((m) => (
          <div key={m.modId} className="row" style={{ cursor: "default" }}>
            <span className="m-main"><Switch checked={on.has(m.modId)} onChange={(v) => setOn((s) => { const n = new Set(s); if (v) n.add(m.modId); else n.delete(m.modId); return n; })}>{m.name}</Switch></span>
            <span className="m-side"><Key size="sm" variant="ghost" onClick={() => void decline(m)} title="Stop asking about this one">Not for me</Key></span>
          </div>
        ))}
      </div>
      <div className="actions" style={{ justifyContent: "flex-end", marginTop: ".6em" }}>
        <Key variant="ghost" onClick={onClose}>Cancel</Key>
        <Key variant="primary" icon="star" busy={busy} disabled={!chosen.length} title={chosen.length ? undefined : "Turn on the ones to endorse"}
          onClick={() => run(async () => { await endorseAll(chosen); onClose(); })}>Endorse {chosen.length}</Key>
      </div>
    </Dialog>
  );
}

/** After a play session (the window comes back 20 minutes or more after Play), one line, at most once a week. */
export function useAfterPlayNudge() {
  useEffect(() => {
    const h = () => {
      const played = read(PLAYED);
      if (!played || Date.now() - played < 20 * 60_000 || Date.now() - read(ASKED) < 7 * 86400_000 || read(SNOOZE) > Date.now()) return;
      write(PLAYED, 0);
      endorse.status().then((s) => {
        if (!s.hasKey || s.candidates.length === 0) return;
        write(ASKED, Date.now());
        toast(`Endorse the ${s.candidates.length} mods you played with?`, false, { label: "Endorse all", run: () => void endorseAll(s.candidates) });
      }).catch(() => {});
    };
    window.addEventListener("focus", h);
    return () => window.removeEventListener("focus", h);
  }, []);
}
export const notePlayed = () => write(PLAYED, Date.now());
