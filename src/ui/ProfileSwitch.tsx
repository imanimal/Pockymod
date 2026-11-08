// The active profile, always in the title bar: one click to switch, make a copy, or start empty.
import { plural } from "../lib/text";
import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";
import { Key } from "./pocket";
import { errorText, library, profiles, type Profile, type SwitchReport } from "../lib/api";
import { toast } from "./toast";
import * as ops from "../lib/ops";

/** Mods a switch couldn't put in by itself (a clash or a copy to allow), said once. */
export function notDeployedText(r: SwitchReport): string | null {
  const n = r.notDeployed.length;
  return n ? `${plural(n, "mod")} need${n === 1 ? "s" : ""} a decision in Library` : null;
}

export function ProfileSwitch({ onManage }: { onManage: () => void }) {
  const [list, setList] = useState<Profile[]>([]);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  // what is running: the id of the profile being switched to, or the key making a new one
  const [busy, setBusy] = useState<number | "copy" | "empty" | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const load = () => profiles.list().then(setList).catch(() => {});
  useEffect(() => {
    load();
    let off: (() => void) | undefined, gone = false;
    library.onChanged(load).then((u) => { if (gone) u(); else off = u; });
    const close = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOpen(false); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => { gone = true; off?.(); document.removeEventListener("mousedown", close); document.removeEventListener("keydown", esc); };
  }, []);
  const active = list.find((p) => p.active);
  const run = async (what: number | "copy" | "empty", f: () => Promise<void>) => {
    setBusy(what);
    try { await f(); } catch (e) { toast(errorText(e), true); } finally { setBusy(null); load(); }
  };
  const switched = (r: SwitchReport) => { const t = notDeployedText(r); if (t) toast(t); };
  const make = (copy: boolean) => run(copy ? "copy" : "empty", async () => {
    const n = name.trim() || `${active?.name ?? "Profile"} ${copy ? "copy" : "new"}`;
    const id = await profiles.create(n, copy ? active?.id : undefined);
    switched(await ops.switchProfile(active, { id, name: n }));
    setName(""); setOpen(false);
  });
  return (
    <div className="pswitch" ref={ref}>
      {/* opening reads the list again: a profile made or deleted in Load order sends no library event */}
      <button type="button" className="pswitch-k" onClick={() => { if (!open) void load(); setOpen(!open); }} aria-expanded={open} aria-busy={busy !== null || undefined} title="Switch profile">
        <Icon name="user" size={9} /><span className="dim">Profile</span><b>{active?.name ?? "Default"}</b><Icon name="down" size={9} />
      </button>
      {open && (
        <div className="pmenu panel" role="menu">
          <div className="ph"><span className="pt"><Icon name="user" />Profiles</span><span className="cnt">{list.length}</span></div>
          <div className="pb">
            {list.map((p) => (
              <button key={p.id} type="button" role="menuitemradio" aria-checked={p.active} aria-busy={busy === p.id || undefined} className={`row${p.active ? " sel" : ""}`} disabled={busy !== null}
                onClick={() => p.active ? setOpen(false) : run(p.id, async () => { switched(await ops.switchProfile(active, p)); setOpen(false); })}>
                <span className="m-main"><span className="m-t">{p.name}</span><span className="m-meta"><span>{plural(p.enabled, "mod")} on</span>{!!p.vortex && <span title="Installed by Vortex">{p.vortex} from Vortex</span>}{!!p.unmanaged && <span className="warnc" title="Added by hand or left behind. Clean up in Library">{p.unmanaged} unmanaged</span>}</span></span>
                {busy === p.id ? <span className="led wait" title="Switching" /> : p.active && <span className="led run" />}
              </button>
            ))}
            <label className="fbox" style={{ flex: "none", marginTop: ".5em" }}><input className="finput" value={name} onChange={(e) => setName(e.target.value)} placeholder="New profile name" aria-label="New profile name"
              onKeyDown={(e) => { if (e.key === "Enter" && busy === null) void make(true); }} /></label>
            <div className="actions" style={{ marginTop: ".4em" }}>
              <Key size="sm" icon="plus" disabled={busy !== null} busy={busy === "copy"} onClick={() => make(true)} title="A new profile with the same mods on, and switch to it">Copy this one</Key>
              <Key size="sm" variant="ghost" disabled={busy !== null} busy={busy === "empty"} onClick={() => make(false)} title="A new profile with every mod off, and switch to it">Start empty</Key>
              <Key size="sm" variant="ghost" onClick={() => { setOpen(false); onManage(); }}>Manage</Key>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
