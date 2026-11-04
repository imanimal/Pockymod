import { useState } from "react";
import { Module, Seg, Switch } from "../ui/pocket";
import { applyTheme, getThemeChoice, setMotion, setThemeChoice, type ThemeChoice } from "../lib/theme";

export function Settings({ onScheme }: { scheme: "cream" | "night"; onScheme: (s: "cream" | "night") => void }) {
  const [theme, setTheme] = useState<ThemeChoice>(getThemeChoice());
  const [motion, setMotionState] = useState(document.documentElement.dataset.motion !== "0");

  return (
    <div className="stack" style={{ maxWidth: "52em" }}>
      <Module icon="settings" title="Look" count="Pocket">
        <div className="fgrid">
          <span className="lab">Theme</span>
          <Seg label="Theme" value={theme} onChange={(t) => { setTheme(t); setThemeChoice(t); onScheme(applyTheme(t)); }}
            options={[{ value: "system", label: "System" }, { value: "cream", label: "Cream" }, { value: "night", label: "Night" }]} />
          <span className="lab">Motion</span>
          <Switch checked={motion} onChange={(v) => { setMotionState(v); setMotion(v); }}>Keys travel and LCDs flicker</Switch>
        </div>
      </Module>
    </div>
  );
}
