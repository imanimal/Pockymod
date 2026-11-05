// A mod's installer options (FOMOD), all on one page: every step that applies is shown at once, a search finds
// any option by name or description across all of them, and the picks so far are counted in the band. Steps that
// depend on earlier picks appear and disappear as you pick. Pick-one groups are single tiles, pick-many groups
// toggle. Defaults puts the installer's own picks back, Recommended picks what the author recommends.
import { useEffect, useMemo, useRef, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { Key, Pill, SearchField } from "../ui/pocket";
import { errorText, library, type Choice, type ChoicesNeeded, type Fomod } from "../lib/api";
import { toast } from "../ui/toast";

const KIND: Record<string, string> = {
  SelectExactlyOne: "pick one", SelectAtMostOne: "pick one or none", SelectAtLeastOne: "pick one or more", SelectAll: "all included", SelectAny: "pick any",
};
const k = (si: number, gi: number, pi: number) => `${si}.${gi}.${pi}`;

/** Which steps show for these picks: a step's conditions see the flags set by the visible steps before it. */
export function visibleSteps(f: Fomod, picked: Set<string>): boolean[] {
  const flags = new Map<string, string>();
  return f.steps.map((s, si) => {
    const v = s.visible;
    const met = (fl: [string, string]) => flags.get(fl[0]) === fl[1];
    const shows = !v || !v.flags.length || (v.any ? v.flags.some(met) : v.flags.every(met));
    if (shows) s.groups.forEach((g, gi) => g.plugins.forEach((p, pi) => { if (picked.has(k(si, gi, pi))) for (const [a, b] of p.flags ?? []) flags.set(a, b); }));
    return shows;
  });
}

/** With `apply`, the dialog changes the options of a mod already installed (its last picks preselected in `need`). */
export function FomodDialog({ need, onClose, onInstalled, apply }: { need: ChoicesNeeded; onClose: () => void; onInstalled?: () => void; apply?: (choices: Choice[]) => Promise<void> }) {
  const f = need.fomod;
  const defaults = useMemo(() => new Set(need.defaults.map((c) => c.join("."))), [need.defaults]);
  const [picked, setPicked] = useState<Set<string>>(() => new Set(defaults));
  const [q, setQ] = useState("");
  const [busy, setBusy] = useState(false);
  const body = useRef<HTMLDivElement>(null);
  const shows = useMemo(() => visibleSteps(f, picked), [f, picked]);
  // a step that just appeared starts picked (its recommended option, else its first), so nothing is left to pick
  useEffect(() => {
    let add: string[] = [];
    f.steps.forEach((s, si) => {
      if (!shows[si]) return;
      s.groups.forEach((g, gi) => {
        if (g.kind !== "SelectExactlyOne" && g.kind !== "SelectAtLeastOne") return;
        if (g.plugins.some((_, pi) => picked.has(k(si, gi, pi)))) return;
        const pi = Math.max(0, g.plugins.findIndex((p) => p.kind === "Recommended" || p.kind === "Required"));
        if (g.plugins[pi] && g.plugins[pi].kind !== "NotUsable") add = [...add, k(si, gi, pi)];
      });
    });
    if (add.length) setPicked((old) => new Set([...old, ...add]));
  }, [f, shows, picked]);

  const choose = (si: number, gi: number, pi: number, kind: string) => setPicked((old) => {
    const n = new Set(old), key = k(si, gi, pi);
    if (kind === "SelectExactlyOne" || kind === "SelectAtMostOne") {
      const had = n.has(key);
      f.steps[si].groups[gi].plugins.forEach((_, i) => n.delete(k(si, gi, i)));
      if (!had || kind === "SelectExactlyOne") n.add(key);
    } else if (kind !== "SelectAll") { if (n.has(key)) n.delete(key); else n.add(key); }
    return n;
  });
  const recommended = () => setPicked((old) => {
    const n = new Set(old);
    f.steps.forEach((s, si) => s.groups.forEach((g, gi) => {
      const rec = g.plugins.map((p, pi) => ({ p, pi })).filter((x) => x.p.kind === "Recommended" || x.p.kind === "Required");
      if (!rec.length) return;
      if (g.kind === "SelectExactlyOne" || g.kind === "SelectAtMostOne") { g.plugins.forEach((_, i) => n.delete(k(si, gi, i))); n.add(k(si, gi, rec[0].pi)); }
      else rec.forEach((x) => n.add(k(si, gi, x.pi)));
    }));
    return n;
  });

  // only the picks in steps that show count
  const live = [...picked].filter((x) => shows[Number(x.split(".")[0])]);
  const missing = f.steps.flatMap((st, si) => (shows[si] ? st.groups.map((g, gi) => ({ g, si, gi, st })) : []))
    .filter(({ g, si, gi }) => (g.kind === "SelectExactlyOne" || g.kind === "SelectAtLeastOne") && !g.plugins.some((_, pi) => picked.has(k(si, gi, pi))));
  const needle = q.trim().toLowerCase();
  const hit = (name: string, desc: string) => !needle || name.toLowerCase().includes(needle) || desc.toLowerCase().includes(needle);
  const matches = needle ? f.steps.reduce((n, s, si) => n + (shows[si] ? s.groups.reduce((m, g) => m + g.plugins.filter((p) => hit(p.name, p.description)).length, 0) : 0), 0) : 0;

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (e.key === "/" && !(e.target as HTMLElement)?.closest?.("input, textarea")) { e.preventDefault(); body.current?.parentElement?.querySelector<HTMLInputElement>(".fomod-top input")?.focus(); }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, []);

  const install = async () => {
    setBusy(true);
    try {
      const choices = live.map((x) => x.split(".").map(Number) as Choice);
      if (apply) { await apply(choices); onClose(); return; }
      const out = await library.installFile({ ...need.request, fomodChoices: choices });
      if (out.status === "installed") toast(`Installed ${need.request.name}${out.needsRedmodDeploy ? ". Deploy REDmods from Load order" : ""}`);
      else if (out.status === "staged") toast(`${need.request.name} clashes with other mods. Decide in Library`, true);
      else if (out.status === "alreadyHave") toast(`Already have ${need.request.name}`);
      onInstalled?.(); onClose();
    } catch (e) { toast(errorText(e), true); setBusy(false); }
  };
  const jump = (si: number) => body.current?.querySelector(`[data-step="${si}"]`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  const visibleCount = shows.filter(Boolean).length;

  return (
    <Dialog icon="tool" title={`${f.name || need.request.name}: options`} count={`${live.length} picked`} onClose={onClose} className="fomod">
      <div className="fomod-top">
        <SearchField value={q} onChange={setQ} placeholder="Find an option" />
        <Key size="sm" variant="ghost" icon="star" onClick={recommended} title="The author's picks in every group">Recommended</Key>
        <Key size="sm" variant="ghost" icon="undo" onClick={() => setPicked(new Set(defaults))} title={apply ? "Back to your current options" : "Back to the installer defaults"}>{apply ? "As installed" : "Defaults"}</Key>
      </div>
      {visibleCount > 2 && !needle && (
        <div className="fomod-jump" aria-label="Steps">
          {f.steps.map((s, si) => shows[si] && <button key={si} type="button" className="chip" onClick={() => jump(si)} title={`Go to ${s.name}`}>{s.name || `Step ${si + 1}`}</button>)}
        </div>
      )}
      {needle && <p className="dim" style={{ fontSize: ".84em" }}>{matches ? `${matches} ${matches === 1 ? "match" : "matches"}` : "No matches"}</p>}
      <div className="fomod-body" ref={body}>
        {f.steps.map((s, si) => {
          if (!shows[si]) return null;
          const groups = s.groups.map((g, gi) => ({ g, gi, list: g.plugins.map((p, pi) => ({ p, pi })).filter(({ p }) => hit(p.name, p.description)) })).filter((x) => x.list.length);
          if (!groups.length) return null;
          return (
            <section key={si} className="fomod-step" data-step={si}>
              {(s.name || f.steps.length > 1) && <h3>{s.name || `Step ${si + 1}`}</h3>}
              {groups.map(({ g, gi, list }) => (
                <div key={gi} className="fgroup">
                  <div className="sub-h">{g.name} <span className="dim">· {KIND[g.kind] ?? "pick any"}</span></div>
                  <div className={`fplugins${g.kind === "SelectExactlyOne" || g.kind === "SelectAtMostOne" ? " one" : ""}`}>
                    {list.map(({ p, pi }) => {
                      const on = picked.has(k(si, gi, pi));
                      return (
                        <button key={pi} type="button" className={`fplugin${on ? " on" : ""}`} disabled={p.kind === "NotUsable" || g.kind === "SelectAll"} aria-pressed={on}
                          onClick={() => choose(si, gi, pi, g.kind)} title={p.description || p.name}>
                          <span className="fplugin-h">{on && <Icon name="check" size={9} />}<b>{p.name}</b>{p.kind === "Recommended" && <Pill tone="ok">Recommended</Pill>}{p.kind === "Required" && <Pill>Required</Pill>}{p.kind === "NotUsable" && <Pill tone="over">Not usable</Pill>}</span>
                          {p.description && <span className="fplugin-d">{p.description}</span>}
                        </button>
                      );
                    })}
                  </div>
                </div>
              ))}
            </section>
          );
        })}
      </div>
      {missing.length > 0 && <div className="note warn">Still to pick {missing.map((m) => `${m.g.name}${m.st.name ? ` (${m.st.name})` : ""}`).join(", ")}</div>}
      <div className="actions" style={{ justifyContent: "flex-end" }}>
        <Key variant="ghost" onClick={onClose}>Cancel</Key>
        <Key variant="primary" icon="install" disabled={busy || missing.length > 0} onClick={install}
          title={missing.length ? "A group still needs a pick" : undefined}>{apply ? "Use these options" : "Install with these"}</Key>
      </div>
    </Dialog>
  );
}
