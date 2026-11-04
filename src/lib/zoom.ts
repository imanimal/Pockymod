// Text size for the whole app: every size is in em from --fs, so one number scales the UI (pixel icons stay
// crisp at their own sizes). Ctrl+= and Ctrl+- step it, Ctrl+0 resets; the choice is remembered.
const KEY = "pm.textSize";
export const SIZES = [12, 13, 14, 15, 16, 18, 20] as const;
const DEFAULT = 14;

export function getTextSize(): number {
  try { const v = Number(localStorage.getItem(KEY)); if (SIZES.includes(v as never)) return v; } catch { /* storage unavailable */ }
  return DEFAULT;
}
export function setTextSize(px: number) {
  document.documentElement.style.setProperty("--fs", `${px}px`);
  try { localStorage.setItem(KEY, String(px)); } catch { /* storage unavailable */ }
  window.dispatchEvent(new CustomEvent("pm-text-size", { detail: px }));
}
export function applyTextSize() { const px = getTextSize(); if (px !== DEFAULT) document.documentElement.style.setProperty("--fs", `${px}px`); }

export function installZoomKeys(onChange?: (px: number) => void): () => void {
  const h = (ev: KeyboardEvent) => {
    if (!(ev.ctrlKey || ev.metaKey) || ev.altKey) return;
    const cur = SIZES.indexOf(getTextSize() as never);
    let next: number | null = null;
    if (ev.key === "=" || ev.key === "+") next = SIZES[Math.min(SIZES.length - 1, cur + 1)];
    else if (ev.key === "-") next = SIZES[Math.max(0, cur - 1)];
    else if (ev.key === "0") next = DEFAULT;
    if (next === null) return;
    ev.preventDefault();
    setTextSize(next);
    onChange?.(next);
  };
  window.addEventListener("keydown", h);
  return () => window.removeEventListener("keydown", h);
}
