// Thin React wrappers over pocket-os.css. They only emit the Pocket classes; all styling lives in the stylesheet.
import type { ReactNode, ButtonHTMLAttributes } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./icons";
import { useOneShot } from "./motion";

type KeyVariant = "primary" | "dark" | "ghost";
type KeySize = "sm" | "lg";

export function Key({ variant, size, icon, latched, children, className = "", ...rest }:
  { variant?: KeyVariant; size?: KeySize; icon?: IconName; latched?: boolean; children?: ReactNode } & ButtonHTMLAttributes<HTMLButtonElement>) {
  const cls = ["btn", variant, size, !children && icon ? "icon" : "", className].filter(Boolean).join(" ");
  return (
    <button type="button" className={cls} aria-pressed={latched === undefined ? undefined : latched} {...rest}>
      {icon && <Icon name={icon} />}
      {children}
    </button>
  );
}

export type Led = "off" | "run" | "wait" | "bad" | "on";

/** A program key: icon, label, optional LED, count badge and progress along its foot (0–1). */
export function ProgramKey({ icon, label, on, led, count, countHot, progress, onClick, title }:
  { icon: IconName; label: string; on?: boolean; led?: Led; count?: number; countHot?: boolean; progress?: number; onClick?: () => void; title?: string }) {
  return (
    <button type="button" className={`nv${on ? " on" : ""}`} onClick={onClick} aria-current={on ? "page" : undefined} title={title ?? label}>
      <Icon name={icon} />
      <span className="lb">{label}</span>
      {led && <span className={`led ${led === "off" ? "" : led}`} aria-label={`status ${led}`} />}
      {count !== undefined && count > 0 && <span className={`ct${countHot ? " hot" : ""}`}>{count}</span>}
      {progress !== undefined && <span className="prog" style={{ ["--p" as string]: String(Math.max(0, Math.min(1, progress))) }} />}
    </button>
  );
}

export type Band = "hot" | "bad";

/** A module: graphite label band (icon, uppercase title, count, tools) over a body. */
export function Module({ icon, title, count, band, tools, children, className = "", bodyClass = "", fill, id }:
  { icon: IconName; title: string; count?: ReactNode; band?: Band; tools?: ReactNode; children?: ReactNode; className?: string; bodyClass?: string; fill?: boolean; id?: string }) {
  return (
    <section id={id} className={["panel", band, fill ? "fill" : "", className].filter(Boolean).join(" ")}>
      <header className="ph">
        <span className="pt"><Icon name={icon} />{title}</span>
        {count !== undefined && <span className="cnt">{count}</span>}
        <span className="sp" />
        {tools}
      </header>
      <div className={`pb ${bodyClass}`.trim()}>{children}</div>
    </section>
  );
}

export type LcdTone = "ok" | "hot" | "bad";

/** An LCD inlay. Numbers belong here. tone: ok (green), hot (amber), bad (red). */
export function Lcd({ label, value, unit, sub, bar, level, tone = "ok", size, className = "" }:
  { label: string; value: ReactNode; unit?: string; sub?: ReactNode; bar?: number; level?: { on: number; of: number }; tone?: LcdTone; size?: "sm" | "big"; className?: string }) {
  const cls = ["display", tone === "ok" ? "" : tone, size, className].filter(Boolean).join(" ");
  // the LCD switches on again (a stepped flicker) when its figure changes, at most every few seconds
  const ref = useOneShot<HTMLDivElement>("boot", typeof value === "string" || typeof value === "number" ? `${value}${unit ?? ""}` : label, 4000);
  return (
    <div ref={ref} className={cls} role="group" aria-label={label}>
      <span className="d-lbl">{label}</span>
      <span className="d-num">{value}{unit && <small>{unit}</small>}</span>
      {sub && <span className="d-sub">{sub}</span>}
      {bar !== undefined && <span className="d-bar"><i style={{ width: `${Math.round(Math.max(0, Math.min(1, bar)) * 100)}%` }} /></span>}
      {level && <LcdLevel on={level.on} of={level.of} />}
    </div>
  );
}

/** A level drawn as LCD blocks, never a thin slider. */
export function LcdLevel({ on, of }: { on: number; of: number }) {
  return (
    <span className="d-lvl" role="meter" aria-valuenow={on} aria-valuemin={0} aria-valuemax={of}>
      {Array.from({ length: of }, (_, i) => <i key={i} className={i < on ? "on" : ""} />)}
    </span>
  );
}

export function Chip({ on, count, children, onClick, disabled, title }:
  { on?: boolean; count?: number; children: ReactNode; onClick?: () => void; disabled?: boolean; title?: string }) {
  return (
    <button type="button" className={`chip${on ? " on" : ""}${count === 0 && !on ? " zero" : ""}`} aria-pressed={!!on} onClick={onClick} disabled={disabled} title={title}>
      {children}
      {count !== undefined && <span className="n">{formatCount(count)}</span>}
    </button>
  );
}

export function Seg<T extends string>({ value, options, onChange, label }:
  { value: T; options: Array<{ value: T; label: string; disabled?: boolean; title?: string }>; onChange: (v: T) => void; label: string }) {
  return (
    <div className="seg" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button key={o.value} type="button" role="radio" aria-checked={o.value === value} className={`chip${o.value === value ? " on" : ""}`}
          disabled={o.disabled} title={o.title} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Switch({ checked, onChange, children, disabled }:
  { checked: boolean; onChange: (v: boolean) => void; children: ReactNode; disabled?: boolean }) {
  return (
    <label className="sw">
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      <span className="sw-t" />
      {children}
    </label>
  );
}

export type PillTone = "ok" | "soon" | "hot" | "over" | "live";
export function Pill({ tone, children }: { tone?: PillTone; children: ReactNode }) {
  return <span className={`pill${tone ? ` ${tone}` : ""}`}>{children}</span>;
}

/** A coloured tag. kind picks a colour from the k-* set (frameworks and mod types). */
export function Tag({ kind, children, square }: { kind?: string; children: ReactNode; square?: boolean }) {
  return <span className={`cchip${kind ? ` k-${kind}` : " none"}${square ? " sq" : ""}`}>{children}</span>;
}

export function SearchField({ value, onChange, placeholder, onEnter, autoFocus }:
  { value: string; onChange: (v: string) => void; placeholder: string; onEnter?: () => void; autoFocus?: boolean }) {
  return (
    <label className="fbox">
      <Icon name="browse" />
      <input className="finput" type="search" value={value} placeholder={placeholder} autoFocus={autoFocus}
        onChange={(e) => onChange(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") onEnter?.(); }} aria-label={placeholder} />
      <kbd>/</kbd>
    </label>
  );
}

export function Empty({ icon, title, children, actions }: { icon: IconName; title: string; children?: ReactNode; actions?: ReactNode }) {
  return (
    <div className="m-empty">
      <Icon name={icon} size={36} className="big" />
      <b>{title}</b>
      {children && <span>{children}</span>}
      {actions && <div className="actions">{actions}</div>}
    </div>
  );
}

export function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1)}M`;
  if (n >= 10_000) return `${Math.round(n / 1000)}k`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}

/** Sizes from bytes, for LCDs and the mono size column. */
export function formatBytes(bytes: number): { value: string; unit: string } {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = bytes, u = 0;
  while (v >= 1024 && u < units.length - 1) { v /= 1024; u++; }
  return { value: v >= 100 || u === 0 ? Math.round(v).toString() : v.toFixed(1), unit: units[u] };
}
export const bytesText = (b: number) => { const f = formatBytes(b); return `${f.value} ${f.unit}`; };
