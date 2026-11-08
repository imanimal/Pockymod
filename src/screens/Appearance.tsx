// Settings, Appearance: pick a theme, its mode, the effects level and motion; import, export and remove themes; and
// make one from the current theme with a live preview. The theme engine is lib/theme.ts; docs/themes.md is the guide.
import { getTextSize, setTextSize } from "../lib/zoom";
import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { Key, Module, Seg, Switch } from "../ui/pocket";
import { Icon } from "../ui/Icon";
import { toast } from "../ui/toast";
import { nope, saved } from "../ui/motion";
import { errorText, pickTextFile, saveTextAs } from "../lib/api";
import {
  BUILTIN_THEMES, COLOR_TOKENS, PALETTES, SIMPLE, customFor, setCustom, exportTheme, installThemePackage, listThemes, previewDraft, previewScheme, readTokens, setFx, setMotion,
  setSchemeChoice, setTheme, themeInfo, toHex, uninstallTheme, useTheme, type FxLevel, type Scheme, type SchemeChoice, type ThemeInfo, type ThemePackage,
} from "../lib/theme";
import "./Appearance.css";

const FX_HINT: Record<FxLevel, string> = {
  off: "No scanlines or flicker",
  subtle: "Faint scanlines, short flicker between views",
  full: "Scanlines, glass and full-screen flicker",
};

const SCHEME_NAME: Record<Scheme, string> = { dark: "Dark", light: "Light" };

export function Appearance() {
  const t = useTheme();
  const [themes, setThemes] = useState<ThemeInfo[]>(BUILTIN_THEMES);
  const [making, setMaking] = useState(false);
  const [custom, setCustomOpen] = useState(false);
  const [removing, setRemoving] = useState<string | null>(null);
  const reload = () => listThemes().then(setThemes).catch(() => {});
  useEffect(() => { void reload(); }, []);
  useEffect(() => { if (!removing) return; const x = setTimeout(() => setRemoving(null), 4000); return () => clearTimeout(x); }, [removing]);

  // an installed theme comes with a notice of the key that goes back (ui/themeEscape.ts)
  const pick = (id: string) => { setTheme(id).catch((e) => toast(errorText(e), true)); };

  const doImport = async (key: HTMLElement) => {
    let f: { text: string } | null = null;
    try { f = await pickTextFile("Import a theme", ["json"]); } catch (e) { toast(errorText(e), true); return; }
    if (!f) return;
    let r: { id: string; name: string };
    try { r = await installThemePackage(f.text); } catch (e) { nope(key); toast(`Theme not installed. ${errorText(e)}`, true); return; }
    await reload();
    toast(`${r.name} installed`);
    pick(r.id);
  };

  const doExport = async () => {
    const { file, text } = exportTheme();
    try { if (await saveTextAs(file, text, ["json"])) toast(`Saved ${file}`); } catch (e) { toast(errorText(e), true); }
  };

  const remove = async (info: ThemeInfo) => {
    if (removing !== info.id) { setRemoving(info.id); return; }
    setRemoving(null);
    try { await uninstallTheme(info.id); await reload(); toast(`${info.name} removed`); } catch (e) { toast(errorText(e), true); }
  };

  const one = t.schemes.length < 2;
  const only = one ? `${t.name} comes in ${t.schemes[0]} only` : undefined;
  const modeOptions: Array<{ value: SchemeChoice; label: string; disabled?: boolean; title?: string }> = [
    { value: "dark", label: "Dark", disabled: one && t.schemes[0] !== "dark", title: only },
    { value: "light", label: "Light", disabled: one && t.schemes[0] !== "light", title: only },
    { value: "system", label: "System", disabled: one, title: only },
  ];

  return (
    <>
      <Module icon="gallery" title="Appearance" count={`${t.name} · ${SCHEME_NAME[t.scheme]}`}>
        <div className="tcards" role="radiogroup" aria-label="Theme">
          {themes.map((th) => (
            <ThemeCard key={th.id} info={th} on={th.id === t.id} scheme={th.id === t.id ? t.scheme : undefined} onPick={() => pick(th.id)}
              onRemove={th.builtin ? undefined : () => void remove(th)} removing={removing === th.id} />
          ))}
        </div>
        <div className="fgrid appear-grid">
          <span className="lab">Mode</span>
          <div className="appear-row">
            <Seg label="Mode" value={one ? t.schemes[0] : t.choice} onChange={setSchemeChoice} options={modeOptions} />
            {one && <span className="dim hint">{only}</span>}
          </div>
          <span className="lab">Effects</span>
          <div className="appear-row">
            <span title={FX_HINT[t.fx]}><Seg label="Effects" value={t.fx} onChange={(v) => setFx(v)}
              options={[{ value: "off", label: "Off" }, { value: "subtle", label: "Subtle" }, { value: "full", label: "Full" }]} /></span>
            {t.fxChoice !== null && t.fxChoice !== t.fxTheme && (
              <button type="button" className="linkish dim hint" onClick={() => setFx(null)}>{t.name} default ({t.fxTheme})</button>)}
          </div>
          <span className="lab">Motion</span>
          <Switch checked={t.motion} onChange={setMotion}>Animations</Switch>
          <span className="lab">Text size</span>
          <TextSize />
        </div>
        <div className="actions appear-actions">
          <Key icon="gallery" variant="dark" onClick={() => setCustomOpen(!custom)} latched={custom}
            title={`Change ${t.name}'s colours`}>Customize {t.name}</Key>
          <Key icon="plus" onClick={() => setMaking(true)} disabled={making} title="New theme, saved as a shareable file">Make your own</Key>
          <Key icon="folder" onClick={(e) => void doImport(e.currentTarget)} title="A theme .json file">Import theme</Key>
          <Key icon="share" onClick={() => void doExport()}>Export theme</Key>
        </div>
      </Module>
      {custom && <Customizer onClose={() => setCustomOpen(false)} onSaveAs={() => { setCustomOpen(false); setMaking(true); }} />}
      {making && <ThemeMaker onDone={() => { setMaking(false); void reload(); }} />}
    </>
  );
}

/** The tokens a sample of an installed theme needs on top of its look: its own values for that scheme. */
function ownTokens(info: ThemeInfo, scheme: Scheme): CSSProperties {
  const p = info.pkg;
  if (!p) return {};
  return { ...(p.tokens ?? {}), ...((scheme === "dark" ? p.dark : p.light) ?? {}) } as CSSProperties;
}

/** A small live sample of a theme: its band, text, keys and an LCD, in its own colours. */
export function ThemeSample({ info, scheme, style }: { info: ThemeInfo; scheme: Scheme; style?: CSSProperties }) {
  return (
    <span className="tprev" data-look={info.look} data-scheme={scheme} style={{ ...ownTokens(info, scheme), ...style }} aria-hidden="true">
      <span className="tp-band"><i className="tp-dot" /><i className="tp-title" /></span>
      <span className="tp-body">
        <span className="tp-lines"><i className="tp-t1" /><i className="tp-t2" /><i className="tp-t3" /></span>
        <span className="tp-lcd">42</span>
      </span>
      <span className="tp-keys"><i className="tp-key" /><i className="tp-key tp-sel" /><i className="tp-key tp-primary" /></span>
    </span>
  );
}

function ThemeCard({ info, on, scheme, onPick, onRemove, removing }:
  { info: ThemeInfo; on: boolean; scheme?: Scheme; onPick: () => void; onRemove?: () => void; removing: boolean }) {
  const both = info.schemes.length > 1;
  const shown: Scheme[] = both ? (scheme ? [scheme, scheme === "dark" ? "light" : "dark"] : info.schemes) : info.schemes;
  return (
    <div className={`tcard${on ? " on" : ""}`}>
      <button type="button" className="tcard-pick" role="radio" aria-checked={on} onClick={onPick} title={`Use ${info.name}`}>
        <span className={`tprev-pair${both ? " two" : ""}`}>{shown.map((s) => <ThemeSample key={s} info={info} scheme={s} />)}</span>
        <span className="tcard-b">
          <span className="tcard-h"><b>{info.name}</b>{on && <Icon name="check" />}</span>
          <span className="tcard-d">{info.description ?? (info.author ? `By ${info.author}` : "Installed theme")}</span>
          <span className="tcard-m">{both ? "Dark and light" : `${SCHEME_NAME[info.schemes[0]]} only`} · {info.builtin ? "built in" : info.author ? `by ${info.author}` : "installed"}</span>
        </span>
      </button>
      {onRemove && (
        <Key size="sm" variant="ghost" icon="trash" className="tcard-x" onClick={onRemove} title={`Remove ${info.name}`}>
          {removing ? "Sure?" : "Remove"}
        </Key>
      )}
    </div>
  );
}

// ── make your own ──

const slug = (name: string) => {
  const s = name.toLowerCase().normalize("NFKD").replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 48);
  if (!s) return "my-theme";
  return BUILTIN_THEMES.some((t) => t.id === s) ? `${s}-theme` : s;
};

type Maps = Partial<Record<Scheme, Record<string, string>>>;

function ThemeMaker({ onDone }: { onDone: () => void }) {
  const t = useTheme();
  const [base] = useState(() => themeInfo(t.id) ?? BUILTIN_THEMES[0]);
  const [startScheme] = useState(t.scheme);
  const [name, setName] = useState(base.builtin ? `My ${base.name}` : base.name);
  const [edit, setEdit] = useState<Scheme>(startScheme);
  const start = useMemo<Maps>(() => {
    const m: Maps = {};
    for (const s of base.schemes) {
      const v = readTokens(base, s);
      m[s] = Object.fromEntries(Object.entries(v).map(([k, x]) => [k, toHex(x)]));
    }
    return m;
  }, [base]);
  const [maps, setMaps] = useState<Maps>(start);
  const box = useRef<HTMLElement>(null);

  // the whole app shows the draft while it is open
  useEffect(() => { previewDraft(maps); }, [maps]);
  useEffect(() => { previewScheme(edit); }, [edit]);
  useEffect(() => () => { previewDraft(null); previewScheme(null); }, []);

  const id = slug(name);
  const replaces = themeInfo(id);

  const pkg = (): ThemePackage => {
    const own = base.pkg;
    const p: ThemePackage = {
      format: "pocket-mods.theme", version: 1, id, name: name.trim() || "My theme",
      description: own?.description ?? `Made from ${base.name}.`,
      scheme: base.schemes.includes(startScheme) ? startScheme : base.schemes[0], base: base.look,
    };
    if (own?.author) p.author = own.author;
    if (base.schemes.length > 1) {
      if (own?.tokens) p.tokens = own.tokens;
      p.dark = { ...(own?.dark ?? {}), ...maps.dark };
      p.light = { ...(own?.light ?? {}), ...maps.light };
    } else {
      p.tokens = { ...(own?.tokens ?? {}), ...maps[base.schemes[0]] };
    }
    if (own?.css) p.css = own.css;
    return p;
  };

  const save = async (key: HTMLElement) => {
    try {
      const text = JSON.stringify(pkg(), null, 2) + "\n";
      const r = await installThemePackage(text);
      previewDraft(null);
      await setTheme(r.id, edit);
      saved(box.current);
      toast(`${r.name} saved`);
      onDone();
    } catch (e) { nope(key); toast(errorText(e), true); }
  };
  const doExport = async () => {
    try { if (await saveTextAs(`${id}.json`, JSON.stringify(pkg(), null, 2) + "\n", ["json"])) toast(`Saved ${id}.json`); } catch (e) { toast(errorText(e), true); }
  };
  const cancel = () => { previewDraft(null); previewScheme(null); onDone(); };

  const cur = maps[edit] ?? {};
  const setTok = (k: string, v: string) => setMaps((m) => ({ ...m, [edit]: { ...(m[edit] ?? {}), [k]: v } }));

  return (
    <section ref={box} className="panel tmake" aria-label="Make your own theme">
      <header className="ph">
        <span className="pt"><Icon name="plus" />Make your own</span>
        <span className="cnt">from {base.name}</span>
        <span className="sp" />
        <Key variant="ghost" size="sm" icon="x" onClick={cancel} aria-label="Close without saving" />
      </header>
      <div className="pb">
        <div className="fgrid appear-grid">
          <label className="lab" htmlFor="tmake-name">Name</label>
          <div className="appear-row">
            <label className="fbox tmake-name"><input id="tmake-name" className="finput" value={name} maxLength={60} onChange={(e) => setName(e.target.value)} spellCheck={false} /></label>
            <span className="dim hint mono">{id}.json{replaces && !replaces.builtin ? " · replaces your theme of this name" : ""}</span>
          </div>
          {base.schemes.length > 1 && (
            <>
              <span className="lab">Editing</span>
              <Seg label="Editing" value={edit} onChange={setEdit} options={base.schemes.map((s) => ({ value: s, label: SCHEME_NAME[s] }))} />
            </>
          )}
        </div>
        <div className="tmake-grid">
          {COLOR_TOKENS.map(({ token, label }) => (
            <label key={token} className="tmake-tok">
              <input type="color" value={cur[token] ?? "#808080"} onChange={(e) => setTok(token, e.target.value)} />
              <span className="tmake-l"><span>{label}</span><code>{token}</code></span>
            </label>
          ))}
        </div>
        <div className="actions">
          <Key variant="primary" icon="check" onClick={(e) => void save(e.currentTarget)}>Save and use</Key>
          <Key icon="share" onClick={() => void doExport()}>Export file</Key>
          <Key variant="ghost" icon="refresh" onClick={() => setMaps(start)}>Start over</Key>
          <Key variant="ghost" onClick={cancel}>Cancel</Key>
        </div>
      </div>
    </section>
  );
}

/** Text size for the whole app (Ctrl+= and Ctrl+- do the same, Ctrl+0 resets). */
function TextSize() {
  const [px, setPx] = useState(getTextSize());
  useEffect(() => { const h = (e: Event) => setPx((e as CustomEvent<number>).detail); window.addEventListener("pm-text-size", h); return () => window.removeEventListener("pm-text-size", h); }, []);
  const opts = [{ value: "13", label: "Small" }, { value: "14", label: "Normal" }, { value: "16", label: "Large" }, { value: "18", label: "Larger" }];
  const value = opts.some((o) => Number(o.value) === px) ? String(px) : "14";
  return (
    <span className="actions" style={{ gap: ".5em" }}>
      <span title="Ctrl+= and Ctrl+-, Ctrl+0 resets"><Seg label="Text size" value={value} onChange={(v) => { setTextSize(Number(v)); setPx(Number(v)); }} options={opts} /></span>
    </span>
  );
}

/** Your colours on the theme in use, kept for that theme and mode: five swatches and palettes, every colour under
 *  Advanced. Each change shows at once; Reset puts the theme's own colours back. */
function Customizer({ onClose, onSaveAs }: { onClose: () => void; onSaveAs: () => void }) {
  const t = useTheme();
  const info = themeInfo(t.id) ?? BUILTIN_THEMES[0];
  const [adv, setAdv] = useState(false);
  const mine = customFor(t.id)[t.scheme] ?? {};
  const base = useMemo(() => {
    const v = readTokens(info, t.scheme);
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, toHex(x)]));
    // read the theme's own colours once per theme and mode, before yours are layered on
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [t.id, t.scheme]);
  const live = (tok: string) => { const v = mine[tok]; return v && /^#/.test(v) ? v : toHex(getComputedStyle(document.documentElement).getPropertyValue(tok).trim() || base[tok] || "#000000"); };
  const put = (tokens: Record<string, string>) => setCustom(t.id, t.scheme, { ...mine, ...tokens });
  const changed = Object.keys(mine).length;
  return (
    <section className="panel tmake" aria-label={`Customize ${t.name}`}>
      <header className="ph">
        <span className="pt"><Icon name="gallery" />Customize {t.name}</span>
        <span className="cnt">{SCHEME_NAME[t.scheme]}{changed ? ` · ${changed} changed` : ""}</span>
        <span className="sp" />
        <Key variant="ghost" size="sm" icon="x" onClick={onClose} aria-label="Close" title="Close, colours stay" />
      </header>
      <div className="pb">
        <div className="cust-swatches">
          {SIMPLE.map((x) => (
            <label key={x.key} className="cust-sw" title={x.tip}>
              <input type="color" value={live(x.key)} onChange={(e) => put(x.tokens(e.target.value))} aria-label={x.label} />
              <span>{x.label}</span>
            </label>
          ))}
        </div>
        <div className="sub-h">Palettes</div>
        <div className="chips">
          {PALETTES.map((p) => (
            <button key={p.name} type="button" className="chip cust-pal" onClick={() => put({ "--accent": p.accent, "--acc2": p.acc2, "--sel": p.acc2, "--focus": p.acc2 })} title={`${p.name} accent and highlight`}>
              <i style={{ background: p.accent }} /><i style={{ background: p.acc2 }} />{p.name}
            </button>
          ))}
        </div>
        <div className="actions" style={{ marginTop: ".6em" }}>
          <Key size="sm" variant="ghost" icon="undo" disabled={!changed} onClick={() => setCustom(t.id, t.scheme, null)} title={`Back to ${t.name}'s own colours`}>Reset</Key>
          <Key size="sm" variant="ghost" icon="chevron" className={adv ? "rot-key" : ""} onClick={() => setAdv(!adv)} aria-expanded={adv} title="Every colour">Advanced</Key>
          <Key size="sm" variant="ghost" icon="plus" onClick={onSaveAs} title="Save these colours as a new theme">Save as a new theme</Key>
        </div>
        {adv && (
          <div className="tmake-grid" style={{ marginTop: ".6em" }}>
            {COLOR_TOKENS.map(({ token, label }) => (
              <label key={token} className="tmake-tok" title={`${label} (${token})`}>
                <input type="color" value={live(token)} onChange={(e) => put({ [token]: e.target.value })} aria-label={label} />
                <span>{label}</span>
                {mine[token] && <button type="button" className="linkish" onClick={() => { const n = { ...mine }; delete n[token]; setCustom(t.id, t.scheme, n); }} title="Theme default">reset</button>}
              </label>
            ))}
          </div>
        )}
      </div>
    </section>
  );
}
