// The component gallery: every Pocket part the app uses, in every variant and state. It's a catalogue, so it
// shows the orange key and the orange band side by side, which a real screen never does.

import { useState } from "react";
import { Deck } from "../components/Deck";
import { ICON_NAMES, Icon } from "../components/Icon";
import { Chip, Empty, Grp, Key, Lcd, Meter, Module, Pill, ProgramKey, SearchField, Seg, Switch, SwitchRow, Tag, SizeChip, Thumb } from "../components/pocket";
import { useApp } from "../lib/store";

export function Gallery() {
  const { go } = useApp();
  const [seg, setSeg] = useState<"a" | "b" | "c">("a");
  const [sw, setSw] = useState(true);
  const [chips, setChips] = useState<string[]>(["CET"]);
  const [text, setText] = useState("");
  const toggle = (c: string) => setChips((x) => (x.includes(c) ? x.filter((y) => y !== c) : [...x, c]));
  return (
    <div className="gallery">
      <Deck>
        <Key icon="settings" onClick={() => go("settings")}>
          Back to Settings
        </Key>
        <Key icon="grid" pressed>
          Gallery
        </Key>
      </Deck>
      <p className="note">The gallery shows every variant at once, so it breaks the one-orange-per-screen rule on purpose.</p>

      <Module title="Keys" icon="chip" count=".btn" auto>
        <div className="actions">
          <Key variant="primary" icon="download">Primary</Key>
          <Key icon="refresh">Default</Key>
          <Key variant="dark" icon="bolt">Dark</Key>
          <Key variant="ghost" icon="link">Ghost</Key>
          <Key pressed icon="star">Pressed</Key>
          <Key disabled icon="close">Disabled</Key>
        </div>
        <div className="actions">
          <Key size="sm" icon="plus">Small</Key>
          <Key size="lg" icon="play">Large</Key>
          <Key size="sm" variant="ghost" icon="close" aria-label="Icon only" />
          <span className="note">Hover, press (3 px travel, springs back), and Tab for the focus ring.</span>
        </div>
      </Module>

      <Module title="Program keys" icon="order" count=".nv" auto>
        <div className="nkeys static">
          <ProgramKey icon="browse" label="Browse" on />
          <ProgramKey icon="library" label="Library" count={3} countHot />
          <ProgramKey icon="download" label="Downloads" led="on" count={5} progress={42} />
          <ProgramKey icon="download" label="Waiting" led="wait" count={2} progress={10} />
          <ProgramKey icon="health" label="Health" count={1} />
        </div>
      </Module>

      <div className="g-grid">
        <Module title="Module" icon="chip" count="12">
          A graphite band: pixel icon, uppercase title, count.
        </Module>
        <Module title="Do this first" icon="download" count="1" band="hot">
          The one orange band on a screen.
        </Module>
        <Module title="Broken" icon="warn" count="2" band="bad">
          A red band when something is broken.
        </Module>
      </div>

      <Module title="LCD inlays" icon="grid" count=".display" auto>
        <div className="lcds">
          <Lcd label="Mods left" value={12} sub="in the queue" level={{ on: 5, of: 12 }} />
          <Lcd label="Needs your click" value={3} tone="hot" sub="Waiting on Nexus" />
          <Lcd label="Requests" value="0" tone="bad" sub="Throttled until 14:00" />
          <Lcd label="Size" value="3.2" unit="GB" sub="to download" bar={0.6} />
        </div>
        <div className="lcds">
          <Lcd label="Big" value="42" size="big" />
          <Lcd label="Small" value="7" size="sm" sub="compact" />
          <div className="lcd-row">
            <span className="lcd-chip">
              <b>01</b>
            </span>
            <span className="lcd-chip hot">
              <b>02</b>
            </span>
            <span className="lcd-chip bad">
              <b>03</b>
            </span>
          </div>
        </div>
        <div className="sect">Level meter outside an LCD</div>
        <Meter on={7} of={12} label="Example level" />
      </Module>

      <Module title="Fields and filters" icon="filter" auto>
        <SearchField value={text} onChange={setText} placeholder="Search field (.fbox)" chips={chips.map((c) => ({ label: c, onRemove: () => toggle(c) }))} />
        <div className="chips">
          {["CET", "RED4ext", "ArchiveXL", "TweakXL"].map((c, i) => (
            <Chip key={c} on={chips.includes(c)} count={[41, 18, 9, 3][i]} onClick={() => toggle(c)}>
              {c}
            </Chip>
          ))}
          <Chip disabled>Disabled</Chip>
        </div>
        <Seg label="Segmented" value={seg} onChange={setSeg} options={[{ value: "a", label: "Downloads" }, { value: "b", label: "Updated", count: 12 }, { value: "c", label: "Size" }]} />
        <SwitchRow checked={sw} onChange={setSw} hint="A hardware toggle, never a thin slider">
          Slide switch (.sw)
        </SwitchRow>
        <div className="actions">
          <Switch checked={false} onChange={() => {}} label="Off" />
          <Switch checked onChange={() => {}} label="On" />
          <Switch checked={false} onChange={() => {}} label="Disabled" disabled />
        </div>
      </Module>

      <Module title="Pills and tags" icon="chip" auto>
        <div className="chips">
          <Pill>Waiting</Pill>
          <Pill tone="soon">Needs your click</Pill>
          <Pill tone="hot">Soon</Pill>
          <Pill tone="over">Failed</Pill>
          <Pill tone="live">Downloading</Pill>
          <Pill tone="ok">Installed</Pill>
        </div>
        <div className="chips">
          <Tag color="var(--k1)">Archive</Tag>
          <Tag color="var(--k2)">Script</Tag>
          <Tag color="var(--k5)">REDmod</Tag>
          <Tag color="var(--k4)">Framework</Tag>
          <Tag color="var(--k3)">Tag 3</Tag>
          <Tag color="var(--k6)">Tag 6</Tag>
        </div>
      </Module>

      <Module title="List rows" icon="library" count=".row" auto bodyClassName="list-body">
        <div className="rows">
          <Grp count="3">Group header</Grp>
          {["Rainy Night City", "HUD Cleaner", "Neon Apartment"].map((n, i) => (
            <div key={n} className={`row mrow brow ${i === 1 ? "sel" : ""}`}>
              <Thumb src={null} alt="" />
              <div className="m-main">
                <span className="m-t">{n}</span>
                <span className="m-meta">
                  <span>author</span>
                  <span>Category</span>
                  <span>12K downloads</span>
                </span>
                <span className="m-tags">
                  <Tag color="var(--k1)">Archive</Tag>
                  <Tag color="var(--k4)">CET</Tag>
                </span>
              </div>
              <span />
              <div className="m-due">
                <SizeChip bytes={340 * 1024 * 1024} />
                <span className="m-abs">3 days ago</span>
              </div>
            </div>
          ))}
        </div>
      </Module>

      <Module title="Icons" icon="grid" count={`${ICON_NAMES.length} · 9×9`} auto>
        <div className="icons">
          {ICON_NAMES.map((n) => (
            <span key={n} className="icon-cell" title={n}>
              <Icon name={n} size={18} />
              <span className="mono">{n}</span>
            </span>
          ))}
        </div>
        <div className="actions">
          {([9, 18, 27, 36, 54] as const).map((s) => (
            <Icon key={s} name="chip" size={s} />
          ))}
        </div>
      </Module>

      <Module title="Empty state" icon="info" auto>
        <Empty icon="browse" title="Nothing here" actions={<Key icon="browse">A key</Key>}>
          Explains what would be here and how to get it.
        </Empty>
      </Module>

      <Module title="Window frame" icon="max" auto>
        <div className="mini-frame">
          <div className="mtop">
            <div className="brandmark">
              <span className="logo">P</span>
              <span>Title bar</span>
            </div>
            <span className="sp" />
            <div className="winkeys">
              <button className="wk" aria-label="Minimize"><Icon name="min" /></button>
              <button className="wk" aria-label="Maximize"><Icon name="max" /></button>
              <button className="wk wk-x" aria-label="Close"><Icon name="close" /></button>
            </div>
          </div>
          <div className="deck">
            <Key icon="refresh">Deck key</Key>
            <Key icon="file">Another</Key>
            <span className="deck-sp" />
            <span className="speaker" />
          </div>
        </div>
      </Module>
    </div>
  );
}
