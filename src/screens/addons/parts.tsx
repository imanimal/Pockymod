// Small pieces of the Add-ons screen: theme swatches, extension icons, what an extension adds and can do, and a
// list of console commands to copy.
import type { CSSProperties, ReactNode } from "react";
import { Icon } from "../../ui/Icon";
import { ICONS, type IconName } from "../../ui/icons";
import { Key } from "../../ui/pocket";
import { toast } from "../../ui/toast";
import { PERMISSIONS, type AddonIcon, type ExtensionManifest, type GroupRule, type Permission } from "../../lib/addon-rules.mjs";
import { themeColors, themeSchemes, type MarketEntry } from "../../lib/market";
import type { ThemePackage } from "../../lib/theme";
import "../../styles/addons.css";

export const isIconName = (v: unknown): v is IconName => typeof v === "string" && v in ICONS;

/** A 9×9 icon: a built-in name or nine rows of nine "." and "#" from an extension. */
export function AddonGlyph({ icon, fallback = "addons", size = 18 }: { icon?: AddonIcon; fallback?: IconName; size?: 9 | 18 | 27 | 36 | 54 }) {
  if (Array.isArray(icon) && icon.length === 9) {
    const rects: Array<[number, number, number]> = [];
    icon.forEach((row, y) => { let x = 0; while (x < 9) { if (row[x] === "#") { let w = 1; while (row[x + w] === "#") w++; rects.push([x, y, w]); x += w; } else x++; } });
    return (
      <svg className={`i${size === 18 ? "" : ` s${size}`}`} viewBox="0 0 9 9" aria-hidden="true" shapeRendering="crispEdges">
        {rects.map(([x, y, w]) => <rect key={`${x}.${y}`} x={x} y={y} width={w} height={1} />)}
      </svg>
    );
  }
  return <Icon name={isIconName(icon) ? icon : fallback} size={size} />;
}

type Colors = ReturnType<typeof themeColors>;
const fromPreview = (p: NonNullable<MarketEntry["preview"]>): Colors => ({
  bg: p.bg ?? "#1a1917", surface: p.surface ?? p.bg ?? "#262522", surface2: p.surface ?? "#302e2a", line: p.text ? `color-mix(in srgb, ${p.text} 18%, ${p.surface ?? p.bg ?? "#262522"})` : "#3f3c36",
  text: p.text ?? "#f4efe5", text3: p.text ? `color-mix(in srgb, ${p.text} 60%, ${p.surface ?? "#262522"})` : "#a39c8d", accent: p.accent ?? "#ff6a2b",
  chrome: p.chrome ?? "#0f0e0d", display: p.display ?? "#45b3a1", displayFg: "rgba(0, 0, 0, .72)",
});

/** A small picture of the app in a theme's colours: title band, rail, a module and an LCD. */
export function ThemeSwatch({ pkg, preview, compact }: { pkg?: Partial<ThemePackage> | null; preview?: MarketEntry["preview"]; compact?: boolean }) {
  const sets: Colors[] = pkg ? themeSchemes(pkg).map((s) => themeColors(pkg, s)) : preview ? [fromPreview(preview)] : [themeColors({}, "dark")];
  return (
    <span className={`tsw${compact ? " compact" : ""}${sets.length > 1 ? " two" : ""}`} aria-hidden="true">
      {sets.map((c, i) => (
        <span key={i} className="tsw-win" style={{ "--t-bg": c.bg, "--t-surface": c.surface, "--t-line": c.line, "--t-text": c.text, "--t-text3": c.text3, "--t-accent": c.accent,
          "--t-chrome": c.chrome, "--t-disp": c.display, "--t-dispfg": c.displayFg } as CSSProperties}>
          <span className="tsw-top"><i className="dot" /><i className="bar" /></span>
          <span className="tsw-body">
            <span className="tsw-rail"><i className="on" /><i /><i /></span>
            <span className="tsw-card">
              <span className="tsw-band" />
              <span className="tsw-line" /><span className="tsw-line s" />
              <span className="tsw-foot"><span className="tsw-key" /><span className="tsw-lcd">88</span></span>
            </span>
          </span>
        </span>
      ))}
    </span>
  );
}

/** The permissions an extension asks for, in words. New ones (not approved before) are marked. */
export function PermissionList({ perms, fresh }: { perms: Permission[]; fresh?: string[] }) {
  if (!perms.length) return <p className="dim ext-none"><Icon name="check" size={9} /> No permissions. Can't reach your mods, Nexus or the internet</p>;
  return (
    <ul className="ext-perms">
      {perms.map((p) => (
        <li key={p}>
          <Icon name={p === "queue.add" || p === "library.toggle" ? "warn" : "info"} size={9} />
          <span><b>{PERMISSIONS[p]?.title ?? p}</b>{fresh?.includes(`permission:${p}`) && <span className="pill soon">New</span>}<span className="dim"> · {PERMISSIONS[p]?.detail}</span></span>
        </li>
      ))}
    </ul>
  );
}

/** Everything an extension adds, item by item. */
export function AddsList({ m, fresh }: { m: ExtensionManifest; fresh?: string[] }) {
  const k = m.contributes;
  const isNew = (prefix: string) => fresh?.some((f) => f.startsWith(prefix)) ? <span className="pill soon">New</span> : null;
  const all: Array<{ icon: IconName; what: string; items: Array<{ key: string; main: string; sub?: string; badge?: ReactNode }> }> = [
    { icon: "browse", what: "Browse shelves", items: (k.shelves ?? []).map((s) => ({ key: s.id, main: s.title, sub: s.hint })) },
    { icon: "library", what: "Library group rules", items: (k.groupRules ?? []).map((r, i) => ({ key: String(i), main: `Into ${r.group}`, sub: ruleText(r.match) })) },
    { icon: "play", what: "Tools in the Play menu", items: (k.tools ?? []).map((t) => ({ key: t.id, main: t.name, sub: t.path ? `Runs ${t.path} in the game folder${t.args?.length ? ` with ${t.args.join(" ")}` : ""}` : `Runs ${t.pick?.file ?? "a program"} you pick${t.args?.length ? ` with ${t.args.join(" ")}` : ""}`, badge: isNew(`tool:${t.id}:`) })) },
    { icon: "play", what: "Launch options", items: (k.launchOptions ?? []).map((o) => ({ key: o.id, main: o.name, sub: `Game with ${o.args.join(" ")}`, badge: isNew(`launch:${o.id}:`) })) },
    { icon: "file", what: "Console command packs", items: (k.commandPacks ?? []).map((p) => ({ key: p.id, main: p.title, sub: `${p.commands.length} commands${p.mods?.length ? `, shown on ${p.mods.length === 1 ? "one mod's" : `${p.mods.length} mods'`} page` : ""}` })) },
    { icon: "link", what: "Links", items: (k.links ?? []).map((l) => ({ key: l.id, main: l.title, sub: hostOf(l.url), badge: isNew(`link:${l.url}`) })) },
    { icon: "addons", what: "Panels", items: (k.panels ?? []).map((p) => ({ key: p.id, main: p.title, sub: "Own page in Add-ons, sandboxed" })) },
  ];
  const rows = all.filter((r) => r.items.length);
  return (
    <div className="ext-adds">
      {rows.map((r) => (
        <div key={r.what}>
          <div className="sub-h"><Icon name={r.icon} size={9} /> {r.what}</div>
          <ul>{r.items.map((i) => <li key={i.key}><b>{i.main}</b>{i.badge}{i.sub && <span className="dim"> · {i.sub}</span>}</li>)}</ul>
        </div>
      ))}
    </div>
  );
}

export const hostOf = (url: string) => { try { return new URL(url).host; } catch { return url; } };
function ruleText(m: GroupRule["match"]): string {
  const parts: string[] = [];
  if (m.name?.length) parts.push(`names with ${m.name.map((x) => `"${x}"`).join(", ")}`);
  if (m.category?.length) parts.push(`category ${m.category.join(", ")}`);
  if (m.author?.length) parts.push(`by ${m.author.join(", ")}`);
  if (m.modType?.length) parts.push(`type ${m.modType.join(", ")}`);
  if (m.frameworks?.length) parts.push(`using ${m.frameworks.join(", ")}`);
  if (m.mods?.length) parts.push(`${m.mods.length} listed mods`);
  return parts.join(" or ");
}

/** Console commands to copy, with a label each. */
export function CommandList({ commands }: { commands: Array<{ command: string; label?: string }> }) {
  const copy = (t: string) => navigator.clipboard.writeText(t).then(() => toast("Copied")).catch(() => toast("Couldn't copy", true));
  return (
    <div className="cmds-list ext-cmds">
      {commands.map((c) => (
        <div key={c.command} className="cmd">
          <span className="cmd-l">{c.label && <span className="dim">{c.label}</span>}<code>{c.command}</code></span>
          <Key size="sm" variant="ghost" icon="file" aria-label={`Copy ${c.command}`} title="Copy" onClick={() => copy(c.command)} />
        </div>
      ))}
    </div>
  );
}
