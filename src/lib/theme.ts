// Theme: Cream (default) or Night, following the system setting unless the user picks one.
export type ThemeChoice = "system" | "cream" | "night";
const KEY = "pm.theme";

export function getThemeChoice(): ThemeChoice {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "cream" || v === "night" || v === "system") return v;
  } catch { /* storage unavailable */ }
  return "system";
}

const media = () => window.matchMedia("(prefers-color-scheme: dark)");

export function applyTheme(choice: ThemeChoice = getThemeChoice()): "cream" | "night" {
  const night = choice === "night" || (choice === "system" && media().matches);
  const root = document.documentElement;
  root.classList.toggle("night", night);
  root.dataset.scheme = night ? "dark" : "light";
  return night ? "night" : "cream";
}

export function setThemeChoice(choice: ThemeChoice) {
  try { localStorage.setItem(KEY, choice); } catch { /* storage unavailable */ }
  applyTheme(choice);
}

/** Re-apply when the system setting changes while the choice is "system". */
export function watchSystemTheme(onChange: () => void): () => void {
  const m = media();
  const h = () => { if (getThemeChoice() === "system") { applyTheme("system"); onChange(); } };
  m.addEventListener("change", h);
  return () => m.removeEventListener("change", h);
}

export function setMotion(on: boolean) {
  document.documentElement.dataset.motion = on ? "1" : "0";
}

// ── installed themes (shared contract; the theme engine fills these in) ──

/** A theme package as a user writes, imports or gets from the marketplace. See docs/themes.md. */
export interface ThemePackage {
  format: "pocket-mods.theme";
  version: 1;
  id: string;
  name: string;
  author?: string;
  description?: string;
  /** The scheme it starts in; a theme with both `dark` and `light` follows the Light/Dark choice. */
  scheme: "dark" | "light";
  /** CSS custom properties, for example { "--bg": "#101014" }, used in both schemes. */
  tokens?: Record<string, string>;
  dark?: Record<string, string>;
  light?: Record<string, string>;
  /** Extra CSS, scoped by the app to html[data-theme="<id>"]. */
  css?: string;
}

/** Validate and save a theme package (JSON text). Returns its id and name. */
export async function installThemePackage(text: string): Promise<{ id: string; name: string }> {
  const pkg = JSON.parse(text) as ThemePackage;
  if (pkg.format !== "pocket-mods.theme" || !pkg.id || !pkg.name) throw new Error("That is not a Pocket Mods theme.");
  const { addons } = await import("./api");
  await addons.save("theme", pkg.id, text);
  return { id: pkg.id, name: pkg.name };
}

export async function uninstallTheme(id: string): Promise<void> {
  const { addons } = await import("./api");
  await addons.remove("theme", id);
}
