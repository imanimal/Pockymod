// A note's "Settings": a link that opens that section of Settings (lib/nav.ts).
import { goSettings } from "../lib/nav";

export function SettingsLink({ section, children }: { section?: string; children?: React.ReactNode }) {
  return <button type="button" className="linkish" onClick={() => goSettings(section)} title={section ? `Settings, ${section}` : "Settings"}>{children ?? "Settings"}</button>;
}
