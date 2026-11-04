// A mod's installer options (FOMOD): every step and group with its pictures and descriptions, the mod's own
// defaults selected. Pick-one groups are radio keys, pick-many groups are switches.
import { useMemo, useState } from "react";
import { Dialog } from "../ui/Dialog";
import { Key, Pill } from "../ui/pocket";
import { errorText, library, type Choice, type ChoicesNeeded } from "../lib/api";
import { toast } from "../ui/toast";

/** With `apply`, the dialog changes the options of a mod already installed (its last picks preselected in `need`). */
export function FomodDialog({ need, onClose, onInstalled, apply }: { need: ChoicesNeeded; onClose: () => void; onInstalled?: () => void; apply?: (choices: Choice[]) => Promise<void> }) {
  const [picked, setPicked] = useState<Set<string>>(() => new Set(need.defaults.map((c) => c.join("."))));
  const [step, setStep] = useState(0);
  const [busy, setBusy] = useState(false);
  const steps = need.fomod.steps;
  const s = steps[step];
  const key = (si: number, gi: number, pi: number) => `${si}.${gi}.${pi}`;
  const choose = (gi: number, pi: number, kind: string) => setPicked((old) => {
    const n = new Set(old);
    const k = key(step, gi, pi);
    if (kind === "SelectExactlyOne" || kind === "SelectAtMostOne") {
      const had = n.has(k);
      s.groups[gi].plugins.forEach((_, i) => n.delete(key(step, gi, i)));
      if (!had || kind === "SelectExactlyOne") n.add(k);
    } else if (kind !== "SelectAll") { n.has(k) ? n.delete(k) : n.add(k); }
    return n;
  });
  const missing = useMemo(() => steps.flatMap((st, si) => st.groups.map((g, gi) => ({ g, si, gi })))
    .filter(({ g, si, gi }) => (g.kind === "SelectExactlyOne" || g.kind === "SelectAtLeastOne") && !g.plugins.some((_, pi) => picked.has(key(si, gi, pi)))), [picked, steps]);
  const install = async () => {
    setBusy(true);
    try {
      const choices = [...picked].map((k) => k.split(".").map(Number) as Choice);
      if (apply) { await apply(choices); onClose(); return; }
      const out = await library.installFile({ ...need.request, fomodChoices: choices });
      if (out.status === "installed") toast(`Installed ${need.request.name}${out.needsRedmodDeploy ? ". Run the REDmod deploy from Load order." : "."}`);
      else if (out.status === "staged") toast(`${need.request.name} is staged: its files clash with other mods. Decide in Library.`, true);
      else if (out.status === "alreadyHave") toast(`You already have ${need.request.name}. Kept yours.`);
      onInstalled?.(); onClose();
    } catch (e) { toast(errorText(e), true); setBusy(false); }
  };
  return (
    <Dialog icon="install" title={`${need.fomod.name || need.request.name}: options`} count={steps.length > 1 ? `step ${step + 1} of ${steps.length}` : undefined} onClose={onClose}>
      {apply && step === 0 && <p className="dim" style={{ fontSize: ".86em" }}>Your last picks are selected. The new ones go in from the files already on disk; nothing is downloaded.</p>}
      {s && <>
        <b className="hi">{s.name}</b>
        {s.groups.map((g, gi) => (
          <div key={gi} className="fgroup">
            <div className="sub-h">{g.name} <span className="dim">· {g.kind === "SelectExactlyOne" ? "pick one" : g.kind === "SelectAtMostOne" ? "pick one or none" : g.kind === "SelectAtLeastOne" ? "pick one or more" : g.kind === "SelectAll" ? "all included" : "pick any"}</span></div>
            <div className="fplugins">
              {g.plugins.map((p, pi) => {
                const on = picked.has(key(step, gi, pi));
                return (
                  <button key={pi} type="button" className={`fplugin${on ? " on" : ""}`} disabled={p.kind === "NotUsable" || g.kind === "SelectAll"} aria-pressed={on} onClick={() => choose(gi, pi, g.kind)}>
                    <span className="fplugin-h"><b>{p.name}</b>{p.kind === "Recommended" && <Pill tone="ok">Recommended</Pill>}{p.kind === "Required" && <Pill>Required</Pill>}{p.kind === "NotUsable" && <Pill tone="over">Not usable</Pill>}</span>
                    {p.description && <span className="fplugin-d">{p.description}</span>}
                  </button>
                );
              })}
            </div>
          </div>
        ))}
      </>}
      {missing.length > 0 && step === steps.length - 1 && <div className="note warn">Still to pick: {missing.map((m) => m.g.name).join(", ")}.</div>}
      <div className="actions" style={{ justifyContent: "flex-end" }}>
        {step > 0 && <Key variant="ghost" onClick={() => setStep(step - 1)}>Back</Key>}
        {step < steps.length - 1 ? <Key variant="dark" icon="chevron" onClick={() => setStep(step + 1)}>Next</Key>
          : <Key variant="primary" icon="install" disabled={busy || missing.length > 0} onClick={install}>{apply ? "Use these options" : "Install with these"}</Key>}
      </div>
    </Dialog>
  );
}
