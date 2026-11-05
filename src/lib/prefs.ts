// Preferences kept in the local database (Settings).
import { settings } from "./api";
import { useSyncExternalStore } from "react";
import type { AiHide } from "./ai";

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

/** Adult mods and collections in Browse: left out (the default), shown with their pictures blurred, or shown. */
export type Adult = "hide" | "blur" | "show";
const ADULT = "browse.adult";
export const ADULT_OPTIONS: Array<{ value: Adult; label: string }> = [{ value: "hide", label: "Hide" }, { value: "blur", label: "Blur" }, { value: "show", label: "Show" }];

export async function adultPref(): Promise<Adult> {
  const v = await settings.get(ADULT).catch(() => null);
  return v === "blur" || v === "show" ? v : "hide";
}

export async function setAdultPref(v: Adult) {
  await settings.set(ADULT, v);
  window.dispatchEvent(new Event("pm-prefs"));
}

/** The adult setting, one copy for every card, kept current when Settings changes it. */
let adultNow: Adult = "hide";
const adultSubs = new Set<() => void>();
let adultLoaded = false;
function adultSubscribe(fn: () => void) {
  adultSubs.add(fn);
  if (!adultLoaded) {
    adultLoaded = true;
    const load = () => { adultPref().then((v) => { adultNow = v; adultSubs.forEach((f) => f()); }).catch(() => {}); };
    load();
    window.addEventListener("pm-prefs", load);
  }
  return () => { adultSubs.delete(fn); };
}
export const useAdult = (): Adult => useSyncExternalStore(adultSubscribe, () => adultNow);

/** AI-tagged mods in Browse: shown (the default), AI-generated ones left out, or every AI-tagged one (lib/ai.ts). */
const AI = "browse.ai";
export async function aiPref(): Promise<AiHide> {
  const v = await settings.get(AI).catch(() => null);
  return v === "generated" || v === "all" ? v : "show";
}
export async function setAiPref(v: AiHide) {
  await settings.set(AI, v);
  window.dispatchEvent(new Event("pm-prefs"));
}
let aiNow: AiHide = "show";
const aiSubs = new Set<() => void>();
let aiLoaded = false;
function aiSubscribe(fn: () => void) {
  aiSubs.add(fn);
  if (!aiLoaded) {
    aiLoaded = true;
    const load = () => { aiPref().then((v) => { if (v !== aiNow) { aiNow = v; aiSubs.forEach((f) => f()); } }).catch(() => {}); };
    load();
    window.addEventListener("pm-prefs", load);
  }
  return () => { aiSubs.delete(fn); };
}
export const useAiHide = (): AiHide => useSyncExternalStore(aiSubscribe, () => aiNow);
