// Keyboard shortcuts for what is used over and over, none of them taken by Windows or the webview: / (or Ctrl+F)
// finds in the screen's search field, Ctrl+1 to Ctrl+9 switch views, ? lists every shortcut, Escape closes, Alt+Left
// goes back, Ctrl+B shows or hides the Nexus pane. Typing in a field is never interrupted (only Escape and Ctrl
// combinations work there). Ctrl+Z/Y (lib/history.ts) and Ctrl+= - 0 (lib/zoom.ts) live with what they change.

export const SHORTCUTS: Array<[string, string]> = [
  ["/", "Find: the search field of the screen or the open window"],
  ["Ctrl+F", "Find, too"],
  ["Ctrl+1 … Ctrl+9", "Browse, Library, Collections, Downloads, Load order, Health, Create, Add-ons, Settings"],
  ["Escape", "Close the window, drawer or menu on top"],
  ["Alt+Left", "Back (a mod or collection page)"],
  ["Ctrl+B", "Show or hide the Nexus pane"],
  ["Ctrl+Z, Ctrl+Y", "Undo, redo"],
  ["Ctrl+=, Ctrl+-, Ctrl+0", "Text size"],
  ["?", "This list"],
];

const typing = (t: EventTarget | null) => {
  const el = t as HTMLElement | null;
  return !!el && (el.isContentEditable || (/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) && !["checkbox", "radio", "button"].includes((el as HTMLInputElement).type)));
};

/** The layer on top: an open window, menu or viewer, else the screen. */
export function topLayer(): ParentNode {
  const layers = [...document.querySelectorAll<HTMLElement>(".lightbox, .ov, .drawer-ov, .pmenu.popover")];
  return layers[layers.length - 1] ?? document.querySelector(".shell > .view") ?? document;
}

/** Focus the search field of the layer on top (the screen's when nothing is open). */
export function focusSearch(): boolean {
  const field = topLayer().querySelector<HTMLInputElement>('input[type="search"], .fbox input');
  if (!field || field.offsetParent === null) return false;
  field.focus();
  field.select();
  return true;
}

export function installKeys(opts: { views: string[]; go: (view: string) => void; help: () => void; back: () => void; pane: () => void }): () => void {
  const h = (e: KeyboardEvent) => {
    const inField = typing(e.target);
    const ctrl = e.ctrlKey || e.metaKey;
    if (ctrl && !e.altKey && !e.shiftKey && /^[1-9]$/.test(e.key)) {
      const v = opts.views[Number(e.key) - 1];
      if (v) { e.preventDefault(); opts.go(v); }
      return;
    }
    if (ctrl && !e.altKey && e.key.toLowerCase() === "f") { if (focusSearch()) e.preventDefault(); return; }
    if (ctrl && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "b") { e.preventDefault(); opts.pane(); return; }
    if (e.altKey && e.key === "ArrowLeft" && !inField) { e.preventDefault(); opts.back(); return; }
    if (inField || ctrl || e.altKey) return;
    if (e.key === "/") { if (focusSearch()) e.preventDefault(); return; }
    if (e.key === "?") { e.preventDefault(); opts.help(); }
  };
  window.addEventListener("keydown", h);
  return () => window.removeEventListener("keydown", h);
}

// ── hover tips ──
// Every `title` in the app shows as the app's own tip (themed, quick, never off screen) instead of the system's slow
// one, on hover and on keyboard or controller focus. The title moves to data-tip while shown, so the two never
// double up.
const TIPPED = "[title], [data-tip], .btn.icon[aria-label], .wk[aria-label]";
export function installTips(): () => void {
  let tip: HTMLDivElement | null = null;
  let timer = 0;
  let cur: HTMLElement | null = null;
  const hide = () => {
    clearTimeout(timer);
    if (cur?.dataset.tip !== undefined && !cur.getAttribute("title")) { cur.setAttribute("title", cur.dataset.tip); delete cur.dataset.tip; }
    cur = null;
    tip?.remove(); tip = null;
  };
  const show = (el: HTMLElement) => {
    // an icon-only key with no title says what it is with its label
    const text = el.getAttribute("title") || el.dataset.tip || el.getAttribute("aria-label");
    if (!text) return;
    hide();
    cur = el;
    if (el.hasAttribute("title") || el.dataset.tip) { el.dataset.tip = text; el.removeAttribute("title"); }
    timer = window.setTimeout(() => {
      if (cur !== el || !el.isConnected) return;
      tip = document.createElement("div");
      tip.className = "apptip";
      tip.setAttribute("role", "tooltip");
      tip.textContent = text;
      (document.querySelector(".shell") ?? document.body).appendChild(tip);
      const r = el.getBoundingClientRect(), t = tip.getBoundingClientRect(), pad = 8;
      let top = r.bottom + 6;
      if (top + t.height > window.innerHeight - pad) top = r.top - t.height - 6;
      const left = Math.max(pad, Math.min(window.innerWidth - t.width - pad, r.left + r.width / 2 - t.width / 2));
      tip.style.left = `${left}px`; tip.style.top = `${Math.max(pad, top)}px`;
    }, 380);
  };
  const over = (e: Event) => {
    const el = (e.target as Element | null)?.closest?.(TIPPED) as HTMLElement | null;
    if (el && el !== cur) show(el);
    else if (!el && cur) hide();
  };
  const focus = (e: FocusEvent) => { const el = (e.target as Element | null)?.closest?.(TIPPED) as HTMLElement | null; if (el && document.documentElement.classList.contains("kbd")) show(el); };
  const out = (e: MouseEvent) => { if (cur && !cur.contains(e.relatedTarget as Node)) hide(); };
  // keyboard or controller in use: focus shows tips and rings; the mouse puts it back
  const kbd = (e: KeyboardEvent) => { if (e.key === "Tab" || e.key.startsWith("Arrow")) document.documentElement.classList.add("kbd"); };
  const mouse = () => document.documentElement.classList.remove("kbd", "pad");
  document.addEventListener("mouseover", over);
  document.addEventListener("mouseout", out);
  document.addEventListener("focusin", focus);
  document.addEventListener("focusout", hide);
  document.addEventListener("mousedown", hide, true);
  document.addEventListener("keydown", kbd, true);
  document.addEventListener("mousemove", mouse);
  window.addEventListener("scroll", hide, true);
  return () => {
    document.removeEventListener("mouseover", over); document.removeEventListener("mouseout", out); document.removeEventListener("focusin", focus);
    document.removeEventListener("focusout", hide); document.removeEventListener("mousedown", hide, true); document.removeEventListener("keydown", kbd, true);
    document.removeEventListener("mousemove", mouse); window.removeEventListener("scroll", hide, true);
  };
}
