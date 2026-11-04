// The active profile, always in the title bar: one click to switch, make a copy, or start empty.
import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";
import { Key } from "./pocket";
import { errorText, library, profiles, type Profile } from "../lib/api";
import { toast } from "./toast";

export function ProfileSwitch({ onManage }: { onManage: () => void }) {
  const [list, setList] = useState<Profile[]>([]);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const load = () => profiles.list().then(setList).catch(() => {});
  useEffect(() => {
    load();
    let off: (() => void) | undefined;
    library.onChanged(load).then((u) => { off = u; });
    const close = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOpen(false); };
    document.addEventListener("mousedown", close);
    return () => { off?.(); document.removeEventListener("mousedown", close); };
  }, []);
  const active = list.find((p) => p.active);
  const run = async (f: () => Promise<void>) => { setBusy(true); try { await f(); } catch (e) { toast(errorText(e), true); } finally { setBusy(false); load(); } };
  const make = (copy: boolean) => run(async () => {
    const id = await profiles.create(name || `${active?.name ?? "Profile"} ${copy ? "copy" : "new"}`, copy ? active?.id : undefined);
    const r = await profiles.activate(id);
    setName(""); setOpen(false);
    toast(`Now on ${name || "the new profile"}${r.removed.length ? `: ${r.removed.length} mods turned off` : ""}.`);
  });
  return (
    <div className="pswitch" ref={ref}>
      <button type="button" className="pswitch-k" onClick={() => setOpen(!open)} aria-expanded={open} title="Profile: which set of mods is on">
        <Icon name="user" size={9} /><span className="dim">Profile</span><b>{active?.name ?? "Default"}</b><Icon name="down" size={9} />
      </button>
      {open && (
        <div className="pmenu panel" role="menu">
          <div className="ph"><span className="pt"><Icon name="user" />Profiles</span><span className="cnt">{list.length}</span></div>
          <div className="pb">
            {list.map((p) => (
              <button key={p.id} type="button" role="menuitemradio" aria-checked={p.active} className={`row${p.active ? " sel" : ""}`} disabled={busy}
                onClick={() => p.active ? setOpen(false) : run(async () => { const r = await profiles.activate(p.id); setOpen(false); toast(`Now on ${p.name}: ${r.deployed.length} mods on, ${r.removed.length} off.`); })}>
                <span className="m-main"><span className="m-t">{p.name}</span><span className="m-meta"><span>{p.enabled} mods on</span></span></span>
                {p.active && <span className="led run" />}
              </button>
            ))}
            <label className="fbox" style={{ flex: "none", marginTop: ".5em" }}><input className="finput" value={name} onChange={(e) => setName(e.target.value)} placeholder="New profile name" aria-label="New profile name" /></label>
            <div className="actions" style={{ marginTop: ".4em" }}>
              <Key size="sm" icon="plus" disabled={busy} onClick={() => make(true)}>Copy this one</Key>
              <Key size="sm" variant="ghost" disabled={busy} onClick={() => make(false)}>Start empty</Key>
              <Key size="sm" variant="ghost" onClick={() => { setOpen(false); onManage(); }}>Manage</Key>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
