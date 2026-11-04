// Component gallery: every Pocket part in every variant and state, in the current theme. The theme strip at the top
// switches the whole app to another theme, scheme or effects level, so each one can be checked part by part.
// It breaks the one-orange-per-screen rule on purpose, to show the orange variants next to the others.
import { useEffect, useState } from "react";
import { Icon } from "../ui/Icon";
import { ICONS, type IconName } from "../ui/icons";
import { Chip, Empty, Key, Lcd, LcdLevel, Module, Pill, ProgramKey, SearchField, Seg, Switch, Tag } from "../ui/pocket";
import { BUILTIN_THEMES, listThemes, setFx, setSchemeChoice, setTheme, useTheme, type ThemeInfo } from "../lib/theme";
import { ThemeSample } from "./Appearance";
import { toast } from "../ui/toast";
import { nope } from "../ui/motion";

const TOKENS = ["--bg", "--bg2", "--surface", "--surface-2", "--line", "--text", "--text-2", "--text-3", "--accent", "--on-accent",
  "--accent-ink", "--acc2", "--chrome-bg", "--chrome-fg", "--disp-bg", "--disp-fg", "--disp-hot", "--disp-bad", "--ok", "--warn", "--bad",
  "--on-bad", "--focus", "--on-ring"];

/** Every installed theme, each a sample you can click; then the scheme and effects of the one showing. */
function ThemeStrip() {
  const t = useTheme();
  const [themes, setThemes] = useState<ThemeInfo[]>(BUILTIN_THEMES);
  useEffect(() => { listThemes().then(setThemes).catch(() => {}); }, []);
  return (
    <Module icon="gallery" title="Themes" count={`${t.name} · ${t.scheme} · effects ${t.fx}`}>
      <div className="gal-themes">
        {themes.flatMap((th) => th.schemes.map((s) => (
          <button key={`${th.id}.${s}`} type="button" className={`gal-theme${t.id === th.id && t.scheme === s ? " on" : ""}`}
            onClick={() => void setTheme(th.id, th.schemes.length > 1 ? s : undefined)} title={`${th.name}, ${s}`}>
            <ThemeSample info={th} scheme={s} />
            <span>{th.name} <span className="dim">{s}</span></span>
          </button>
        )))}
      </div>
      <div className="actions" style={{ marginTop: ".8em" }}>
        <Seg label="Scheme" value={t.choice} onChange={setSchemeChoice}
          options={(["dark", "light", "system"] as const).map((v) => ({ value: v, label: v[0].toUpperCase() + v.slice(1), disabled: t.schemes.length < 2 }))} />
        <Seg label="Effects" value={t.fx} onChange={(v) => setFx(v)} options={[{ value: "off", label: "Effects off" }, { value: "subtle", label: "Subtle" }, { value: "full", label: "Full" }]} />
        <Key size="sm" onClick={(e) => { nope(e.currentTarget); toast("Refused: this is what a refused action looks like.", true); }}>Try a refused action</Key>
        <Key size="sm" onClick={() => toast("Saved. Toasts slide in, drain and slide out.")}>Show a toast</Key>
      </div>
    </Module>
  );
}

// The spec's component names and the Pocket class each one is built on.
const NAMES: Array<[spec: string, pocket: string, note: string]> = [
  [".k", ".btn", "key: .primary .dark .ghost .sm .lg; .on or aria-pressed latched; .down pressed"],
  [".pk", ".nv", "program key: .on latched, .led, .ct count badge, .prog foot progress"],
  [".mod / .band", ".panel / .ph", "module and label band: .pt title, .cnt count; .panel.hot orange, .panel.bad red"],
  [".lcd", ".display", "LCD inlay: .d-lbl .d-num .d-sub .d-bar; .hot .bad .sm .big"],
  [".lvl", ".d-lvl", "LCD block level meter inside a .display"],
  [".srch", ".fbox", "search field with .finput"],
  [".chip", ".chip", "filter chip; .on applied; .n count"],
  [".seg", ".seg", "segmented control of chips"],
  [".sw", ".sw", "slide switch (added, not in the reference)"],
  [".pill", ".duepill / .pill", "status pill: .ok .soon .hot .over .live"],
  [".cc", ".cchip", "coloured tag; --kc or a .k-* colour"],
  [".frow / .grp", ".row / .grp", "list row and group header; .row.modrow for mods"],
  ["window frame", ".win / .mtop / .winkeys / .deck", "title bar, window keys, key deck"],
];

export function Gallery() {
  const [chips, setChips] = useState<Record<string, boolean>>({ cet: true, redscript: false, archivexl: true, tweakxl: false });
  const [seg, setSeg] = useState<"downloads" | "endorsements" | "updated">("downloads");
  const [sw1, setSw1] = useState(true);
  const [sw2, setSw2] = useState(false);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(1);

  return (
    <div className="stack gallery">
      <div className="grp" style={{ paddingTop: 0 }}>Component gallery <span>pocket-os.css</span></div>
      <p className="dim" style={{ marginTop: -6 }}>Every part in every state. This screen alone shows orange more than once, so the variants sit side by side.</p>
      <ThemeStrip />

      <div className="grid2">
        <Module icon="gallery" title="Keys" count=".btn">
          <div className="sub-h">Variants</div>
          <div className="actions">
            <Key>Default</Key>
            <Key variant="primary" icon="install">Install</Key>
            <Key variant="dark" icon="settings">Dark</Key>
            <Key variant="ghost">Ghost</Key>
            <Key size="sm">Small</Key>
            <Key size="lg" icon="play">Large</Key>
            <Key icon="refresh" aria-label="Refresh" />
          </div>
          <div className="sub-h">States</div>
          <div className="actions">
            <Key>Rest</Key>
            <Key className="hov">Hover</Key>
            <Key className="down">Pressed</Key>
            <Key latched icon="filter">Latched</Key>
            <Key className="focus">Focus</Key>
            <Key disabled>Disabled</Key>
          </div>
          <div className="sub-h">Primary states</div>
          <div className="actions">
            <Key variant="primary">Rest</Key>
            <Key variant="primary" className="hov">Hover</Key>
            <Key variant="primary" className="down">Pressed</Key>
            <Key variant="primary" className="focus">Focus</Key>
          </div>
        </Module>

        <Module icon="gallery" title="Program keys" count=".nv">
          <div className="nkeys rail" style={{ maxWidth: "15em" }}>
            <ProgramKey icon="browse" label="Browse" on />
            <ProgramKey icon="downloads" label="Downloads" led="run" count={7} progress={0.42} />
            <ProgramKey icon="library" label="Library" count={3} countHot title="3 updates" />
            <ProgramKey icon="health" label="Health" led="bad" />
            <ProgramKey icon="settings" label="Settings" led="off" />
          </div>
          <p className="dim" style={{ marginTop: ".8em", fontSize: ".86em" }}>
            Latched key is graphite with an orange icon. LED: off, running (stepped blink), waiting, problem. The bar along the foot is the queue's progress.
          </p>
        </Module>
      </div>

      <div className="grid3">
        <Module icon="library" title="Graphite band" count="12 mods">
          <p>Every module wears a band with an icon, an uppercase title and a count.</p>
        </Module>
        <Module icon="install" title="Do this first" count="1" band="hot">
          <p>The orange band. Exactly one per screen, on the primary task.</p>
        </Module>
        <Module icon="warn" title="Problem" count="2 issues" band="bad" tools={<Key size="sm" variant="ghost">Details</Key>}>
          <p>A red band when something is broken.</p>
        </Module>
      </div>

      <Module icon="downloads" title="LCD inlays" count=".display">
        <div className="lcds">
          <Lcd label="Mods left" value="12" sub="of 31 in the queue" bar={0.61} />
          <Lcd label="Needs your click" value="7" sub="free account" tone="hot" />
          <Lcd label="Throttled" value="429" sub="resets 00:00 GMT" tone="bad" />
          <Lcd label="Requests left" value="487" unit="/HR" level={{ on: 7, of: 10 }} sub="hourly budget" />
        </div>
        <div className="lcds" style={{ marginTop: ".6em", alignItems: "center" }}>
          <Lcd label="Size" value="34.1" unit="MB" size="sm" />
          <Lcd label="Queue" value="03" size="sm" tone="hot" />
          <Lcd label="Time left" value="4:12" size="big" />
          <div className="display" style={{ flex: "0 1 14em" }}>
            <span className="d-lbl">Level</span>
            <LcdLevel on={3} of={8} />
          </div>
        </div>
      </Module>

      <div className="grid2">
        <Module icon="filter" title="Fields and chips" count=".fbox .chip .seg">
          <SearchField value={q} onChange={setQ} placeholder="Search Cyberpunk 2077 mods" />
          <div className="sub-h">Filter chips with counts</div>
          <div className="chips">
            {([["cet", "Cyber Engine Tweaks", 766], ["redscript", "redscript", 1204], ["archivexl", "ArchiveXL", 7761], ["tweakxl", "TweakXL", 0]] as const).map(([k, label, n]) => (
              <Chip key={k} on={chips[k]} count={n} onClick={() => setChips({ ...chips, [k]: !chips[k] })}>{label}</Chip>
            ))}
          </div>
          <div className="sub-h">Segmented control</div>
          <Seg label="Sort" value={seg} onChange={setSeg} options={[{ value: "downloads", label: "Downloads" }, { value: "endorsements", label: "Endorsements" }, { value: "updated", label: "Updated" }]} />
        </Module>

        <Module icon="settings" title="Switches, pills, tags" count=".sw .pill .cchip">
          <div className="stack" style={{ gap: ".6em" }}>
            <Switch checked={sw1} onChange={setSw1}>Deploy with hard links</Switch>
            <Switch checked={sw2} onChange={setSw2}>Show adult content</Switch>
            <Switch checked={false} onChange={() => {}} disabled>Publish collections (experiment)</Switch>
          </div>
          <div className="sub-h">Status pills</div>
          <div className="actions">
            <Pill>Not downloaded</Pill>
            <Pill tone="ok">Installed</Pill>
            <Pill tone="soon">Update soon</Pill>
            <Pill tone="hot">Waiting on you</Pill>
            <Pill tone="over">Conflict</Pill>
            <Pill tone="live">Installing</Pill>
          </div>
          <div className="sub-h">Coloured tags</div>
          <div className="actions">
            <Tag kind="cet">CET</Tag><Tag kind="red4ext">RED4ext</Tag><Tag kind="redscript">redscript</Tag><Tag kind="archivexl">ArchiveXL</Tag>
            <Tag kind="tweakxl">TweakXL</Tag><Tag kind="codeware">Codeware</Tag><Tag kind="cybercmd">cybercmd</Tag>
          </div>
          <div className="actions" style={{ marginTop: ".4em" }}>
            <Tag kind="redmod" square>REDmod</Tag><Tag kind="archive" square>Archive</Tag><Tag kind="script" square>Script</Tag><Tag kind="framework" square>Framework</Tag><Tag square>Untagged</Tag>
          </div>
        </Module>
      </div>

      <div className="grid2">
        <Module icon="browse" title="List rows" count=".row .grp" bodyClass="list">
          <div className="rows">
            <div className="grp">Frameworks <span>3</span></div>
            {[
              { n: "Cyber Engine Tweaks", a: "yamashi", t: [["framework", "Framework"]], s: "33.3 MB", d: "26 Oct" },
              { n: "ArchiveXL", a: "psiberx", t: [["red4ext", "RED4ext"], ["framework", "Framework"]], s: "1.0 MB", d: "29 Oct" },
              { n: "TweakXL", a: "psiberx", t: [["red4ext", "RED4ext"], ["framework", "Framework"]], s: "0.9 MB", d: "28 Oct" },
            ].map((r, i) => (
              <button key={r.n} type="button" className={`row modrow${sel === i ? " sel" : ""}`} onClick={() => setSel(i)}>
                <span className="thumb ph-img"><Icon name="chip" /></span>
                <span className="m-main">
                  <span className="m-t">{r.n}</span>
                  <span className="m-meta"><span>{r.a}</span><span>Modders resources</span></span>
                  <span className="m-tags">{r.t.map(([k, l]) => <Tag key={l} kind={k} square={k === "framework"}>{l}</Tag>)}</span>
                </span>
                <span className="m-side"><span>{r.s}</span><span>{r.d}</span></span>
              </button>
            ))}
            <div className="grp">Hidden <span>1</span></div>
            <div className="row modrow off">
              <span className="thumb ph-img"><Icon name="hide" /></span>
              <span className="m-main"><span className="m-t">A mod you hid</span><span className="m-meta"><span>someone</span><span>Hidden locally</span></span></span>
              <span className="m-side"><span>2.1 MB</span></span>
            </div>
          </div>
        </Module>

        <Module icon="gallery" title="Window frame" count=".win .mtop .deck">
          <div className="win">
            <div className="mtop">
              <span className="brandmark"><span className="logo">P</span><span>Pocket Mods</span></span>
              <span className="crumb"><span className="v">Browse</span><span className="ctx">Cyberpunk 2077 · 2.31</span></span>
              <span className="sp" />
              <span className="winkeys">
                <button type="button" className="wk" aria-label="Minimise"><Icon name="minus" size={9} /></button>
                <button type="button" className="wk" aria-label="Maximise"><Icon name="max" size={9} /></button>
                <button type="button" className="wk wk-x" aria-label="Close"><Icon name="x" size={9} /></button>
              </span>
            </div>
            <div className="wbody">
              <Empty icon="browse" title="Nothing here yet">Views sit on the dot grid like devices on a mat.</Empty>
              <div className="deck">
                <Key latched icon="browse">Mods</Key>
                <Key icon="collections">Collections</Key>
                <span className="deck-sp" data-model="Pocket PM-1" />
                <span className="speaker" />
              </div>
            </div>
          </div>
        </Module>
      </div>

      <div className="grid2">
        <Module icon="gallery" title="Icons" count={`${Object.keys(ICONS).length} · 9×9`}>
          <div className="chips" style={{ gap: ".7em", color: "var(--text)" }}>
            {(Object.keys(ICONS) as IconName[]).map((n) => (
              <span key={n} title={n} style={{ display: "inline-flex", flexDirection: "column", alignItems: "center", gap: 4, width: 52 }}>
                <Icon name={n} />
                <span className="mono dim" style={{ fontSize: 9 }}>{n}</span>
              </span>
            ))}
          </div>
          <div className="sub-h">Whole 9 px sizes</div>
          <div className="actions" style={{ color: "var(--text)", alignItems: "flex-end" }}>
            {([9, 18, 27, 36, 54] as const).map((s) => <Icon key={s} name="health" size={s} />)}
          </div>
        </Module>

        <Module icon="settings" title="Tokens and type" count={`${TOKENS.length} tokens`}>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(8.5em, 1fr))", gap: ".4em" }}>
            {TOKENS.map((t) => (
              <span key={t} style={{ display: "flex", alignItems: "center", gap: ".5em", fontSize: ".8em" }}>
                <span style={{ width: 18, height: 18, borderRadius: 4, background: `var(${t})`, boxShadow: "inset 0 0 0 1px var(--line)" }} />
                <code>{t}</code>
              </span>
            ))}
          </div>
          <div className="sub-h">Type</div>
          <h2 style={{ fontSize: "1.5em" }}>Space Grotesk, titles and body</h2>
          <p className="mono" style={{ fontSize: ".85em", marginTop: ".3em" }}>Space Mono: bands, legends, 34.1 MB</p>
          <div className="display sm inline" style={{ display: "inline-flex", marginTop: ".6em" }}><span className="d-lbl">Doto</span><span className="d-num">0123456789</span></div>
        </Module>
      </div>

      <Module icon="info" title="Spec names" count={`${NAMES.length} parts`}>
        <p className="dim" style={{ marginBottom: ".6em" }}>The build spec names components .k, .pk, .mod and so on. The Pocket stylesheet is the source of truth, so the app uses its classes:</p>
        <div className="fgrid" style={{ gridTemplateColumns: "10em 15em minmax(0,1fr)" }}>
          {NAMES.map(([s, p, n]) => [<code key={s + "s"}>{s}</code>, <code key={s + "p"}>{p}</code>, <span key={s + "n"} style={{ fontSize: ".86em" }}>{n}</span>])}
        </div>
      </Module>

      <Module icon="downloads" title="Loading and feedback" count=".skel .nope .boot">
        <div className="mgrid">{Array.from({ length: 3 }, (_, i) => <span key={i} className="mcard panel skel" style={{ height: "9em" }} />)}</div>
        <div className="stack" style={{ gap: ".4em", marginTop: ".8em" }}>
          <span className="skel skel-line" style={{ width: "60%" }} />
          <span className="skel skel-line" style={{ width: "85%" }} />
        </div>
        <div className="note bad" style={{ marginTop: ".8em" }}>A problem note shakes once as it appears.</div>
      </Module>

      <div className="grid2">
        <section className="panel dialog" style={{ width: "auto", animation: "none" }}>
          <header className="ph"><span className="pt"><Icon name="warn" />Dialog</span><span className="sp" /><Key variant="ghost" size="sm" icon="x" aria-label="Close" /></header>
          <div className="pb">
            <p>Dialogs are modules on a solid scrim. Nothing is blurred.</p>
            <div className="note warn">Game is under Program Files (x86). Writes may be blocked.</div>
            <div className="actions"><Key variant="ghost">Cancel</Key><Key>Choose a folder</Key></div>
          </div>
        </section>
        <div className="stack" style={{ alignItems: "flex-start" }}>
          <div className="toast" style={{ animation: "none" }}><Icon name="check" /><span>Installed ArchiveXL 1.28.6</span><Key size="sm" variant="ghost">Undo</Key></div>
          <div className="toast bad" style={{ animation: "none" }}><Icon name="warn" /><span>Download failed: link expired</span><Key size="sm">Retry</Key></div>
          <div className="log">{"redMod deploy\n> 4 mods compiled\n> ok"}</div>
        </div>
      </div>
    </div>
  );
}
