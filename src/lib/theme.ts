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
