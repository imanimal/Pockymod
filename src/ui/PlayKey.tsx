// Play (in the launch deck): checks Health first and says what is broken; runs the REDmod deploy if needed; starts the game modded.
// Extensions can add launch options, tools and console commands; they are in the menu beside the key (PlayMenu).
import { useEffect, useRef, useState } from "react";
import { Dialog } from "./Dialog";
import { Icon } from "./Icon";
import { Key } from "./pocket";
import { errorText, extensionsApi, game, launchGame, pickProgram, type Issue } from "../lib/api";
import { setToolPath, useExtensions, type Contributions } from "../lib/extensions";
import { CommandList } from "../screens/addons/parts";
import { toast } from "./toast";

/** A launch option from an extension. */
export interface LaunchChoice { extId: string; id: string; name: string }

export function PlayKey({ onHealth }: { onHealth: () => void }) {
  const [problems, setProblems] = useState<Issue[] | null>(null);
  const [pending, setPending] = useState<LaunchChoice | null>(null);
  const [busy, setBusy] = useState(false);
  const start = async (opt?: LaunchChoice | null) => {
    setBusy(true);
    try {
      const r = opt ? await extensionsApi.launch(opt.extId, opt.id) : await launchGame();
      if (!r.started && r.redmod) toast(`The REDmod deploy reported problems: ${r.redmod.problems[0] ?? "see Load order"}. The game was not started.`, true);
      else toast(`${r.modded ? "Starting Cyberpunk 2077 with REDmods" : "Starting Cyberpunk 2077"}${opt ? `: ${opt.name}` : ""}.`);
      setProblems(null);
    } catch (e) { toast(errorText(e), true); } finally { setBusy(false); }
  };
  const check = async (opt?: LaunchChoice) => {
    setBusy(true);
    try {
      const h = await game.health();
      const bad = h.issues.filter((i) => i.severity === "problem");
      if (bad.length) { setProblems(bad); setPending(opt ?? null); setBusy(false); return; }
      await start(opt);
    } catch { await start(opt); }
  };
  return (
    <span className="playkey">
      <Key variant="dark" icon="play" className="play-key" onClick={() => void check()} disabled={busy} title="Check for problems, then start the game with your mods">{busy ? "Starting" : "Play"}</Key>
      <PlayMenu onLaunch={(o) => void check(o)} disabled={busy} />
      {problems && (
        <Dialog icon="warn" title="Before you play" count={`${problems.length} problems`} band="bad" onClose={() => setProblems(null)}>
          <div className="rows">{problems.slice(0, 5).map((p) => <div key={p.id} className="row" style={{ cursor: "default" }}><span className="m-main"><span className="m-t" style={{ whiteSpace: "normal" }}>{p.title}</span><span style={{ fontSize: ".86em" }}>{p.detail}</span></span></div>)}</div>
          <div className="actions" style={{ justifyContent: "flex-end" }}>
            <Key variant="ghost" disabled={busy} onClick={() => void start(pending)}>Play anyway</Key>
            <Key onClick={() => { setProblems(null); onHealth(); }}>Fix them first</Key>
          </div>
        </Dialog>
      )}
    </span>
  );
}

/** Start a tool an extension lists; asks for the program first when the user has not picked it yet. */
export async function startTool(t: Contributions["tools"][number]) {
  try {
    if (!t.ready) {
      const p = await pickProgram(t.pick?.title ?? `Find ${t.pick?.file ?? t.name}`);
      if (!p) return;
      await setToolPath(t.extId, t.id, p);
    }
    await extensionsApi.startTool(t.extId, t.id);
    toast(`Starting ${t.name}.`);
  } catch (e) { toast(errorText(e), true); }
}

/**
 * The menu beside Play: launch options and tools from extensions, and their console commands. Shows nothing when no
 * extension adds any. `onLaunch` starts the game with an option (PlayKey runs the health check first).
 */
export function PlayMenu({ onLaunch, disabled }: { onLaunch: (o: LaunchChoice) => void; disabled?: boolean }) {
  const { active } = useExtensions();
  const [open, setOpen] = useState(false);
  const [cmds, setCmds] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOpen(false); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => { document.removeEventListener("mousedown", close); document.removeEventListener("keydown", esc); };
  }, [open]);
  const { launchOptions, tools, commandPacks } = active;
  if (!launchOptions.length && !tools.length && !commandPacks.length) return null;
  return (
    <span ref={ref} style={{ position: "relative", display: "inline-flex" }}>
      <Key size="sm" variant="dark" icon="down" onClick={() => setOpen(!open)} aria-expanded={open} aria-label="More ways to play" title="Launch options and tools" disabled={disabled} />
      {open && (
        <div className="playmenu panel" role="menu">
          <div className="ph"><span className="pt"><Icon name="play" />Play</span><span className="cnt">from your add-ons</span></div>
          <div className="pb">
            {launchOptions.length > 0 && <div className="grp">Start the game with</div>}
            {launchOptions.map((o) => (
              <button key={`${o.extId}/${o.id}`} type="button" role="menuitem" className="row" onClick={() => { setOpen(false); onLaunch({ extId: o.extId, id: o.id, name: o.name }); }} title={o.description ?? o.extName}>
                <span className="m-main"><span className="m-t">{o.name}</span><span className="m-meta"><span><code>{o.args.join(" ")}</code></span><span>{o.extName}</span></span></span>
                <Icon name="play" size={9} />
              </button>
            ))}
            {tools.length > 0 && <div className="grp">Tools</div>}
            {tools.map((t) => (
              <button key={`${t.extId}/${t.id}`} type="button" role="menuitem" className="row" onClick={() => { setOpen(false); void startTool(t); }} title={t.description ?? t.extName}>
                <span className="m-main"><span className="m-t">{t.name}</span><span className="m-meta"><span>{t.ready ? (t.path ? "In the game folder" : "Your copy") : `Choose ${t.pick?.file ?? "the program"} first`}</span><span>{t.extName}</span></span></span>
                <Icon name={t.ready ? "chevron" : "folder"} size={9} />
              </button>
            ))}
            {commandPacks.length > 0 && <div className="grp">In game</div>}
            {commandPacks.length > 0 && (
              <button type="button" role="menuitem" className="row" onClick={() => { setOpen(false); setCmds(true); }}>
                <span className="m-main"><span className="m-t">Console commands</span><span className="m-meta"><span>{commandPacks.reduce((a, p) => a + p.commands.length, 0)} to copy</span><span>Cyber Engine Tweaks</span></span></span>
                <Icon name="file" size={9} />
              </button>
            )}
          </div>
        </div>
      )}
      {cmds && (
        <Dialog icon="file" title="Console commands" count={`${commandPacks.length} packs`} onClose={() => setCmds(false)}>
          <p className="dim">Copy one, then paste it in the Cyber Engine Tweaks console in game.</p>
          {commandPacks.map((p) => (
            <div key={`${p.extId}/${p.id}`} className="stack" style={{ gap: ".3em" }}>
              <div className="sub-h">{p.title} <span className="dim">· {p.extName}</span></div>
              <CommandList commands={p.commands} />
            </div>
          ))}
        </Dialog>
      )}
    </span>
  );
}
