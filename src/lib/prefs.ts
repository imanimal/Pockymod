// Install preferences, kept in the local database (Settings).
import { settings } from "./api";

export type Requirements = "always" | "ask" | "never";
export interface InstallPrefs { requirements: Requirements; confirm: boolean }

export async function installPrefs(): Promise<InstallPrefs> {
  const [r, c] = await Promise.all([settings.get("install.requirements").catch(() => null), settings.get("install.confirm").catch(() => null)]);
  return { requirements: r === "never" || r === "ask" ? r : "always", confirm: c !== "0" };
}

export async function setInstallPrefs(p: Partial<InstallPrefs>) {
  if (p.requirements) await settings.set("install.requirements", p.requirements);
  if (p.confirm !== undefined) await settings.set("install.confirm", p.confirm ? "1" : "0");
  window.dispatchEvent(new Event("pm-prefs"));
}
