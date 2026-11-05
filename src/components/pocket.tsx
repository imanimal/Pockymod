// Pocket components as React. Class names are the Pocket stylesheet's (see the map at the top of pocket-os.css).

import { useRef, type ButtonHTMLAttributes, type CSSProperties, type ReactNode } from "react";
import { bytesParts } from "../lib/format";
import { Icon, type IconName } from "./Icon";

function cx(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(" ");
}

/** Plays the key's spring-back (.ack) when it's released. */
function useAck<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  const ack = () => {
    const el = ref.current;
    if (!el) return;
    el.classList.remove("ack");
    void el.offsetWidth;
    el.classList.add("ack");
  };
  const done = () => ref.current?.classList.remove("ack");
  return { ref, ack, done };
}

type KeyProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "dark" | "ghost";
  size?: "sm" | "lg";
  icon?: IconName;
  pressed?: boolean;
};

/** A raised hardware key with 3 px of travel. Exactly one `primary` (orange) key per screen. */
export function Key({ variant, size, icon, pressed, className, children, onPointerUp, onAnimationEnd, ...rest }: KeyProps) {
  const a = useAck<HTMLButtonElement>();
  return (
    <button
      ref={a.ref}
      type="button"
      className={cx("btn", variant, size, className)}
      aria-pressed={pressed}
      onPointerUp={(e) => {
        a.ack();
        onPointerUp?.(e);
      }}
      onAnimationEnd={(e) => {
        a.done();
        onAnimationEnd?.(e);
      }}
      {...rest}
    >
      {icon ? <Icon name={icon} /> : null}
      {children != null ? <span className="bl">{children}</span> : null}
    </button>
  );
}

/** A program key on the view rail: LED for a running task, a progress foot, a count badge. */
export function ProgramKey({
  icon,
  label,
  on,
  count,
  countHot,
  led,
  progress,
  onClick,
}: {
  icon: IconName;
  label: string;
  on?: boolean;
  count?: number;
  countHot?: boolean;
  led?: "on" | "wait" | "off";
  progress?: number | null;
  onClick?: () => void;
}) {
  const a = useAck<HTMLButtonElement>();
  return (
    <button
      ref={a.ref}
      type="button"
      className={cx("nv", on && "on")}
      aria-current={on ? "page" : undefined}
      onClick={onClick}
      onPointerUp={a.ack}
      onAnimationEnd={a.done}
    >
      <Icon name={icon} />
      <span className="lb">{label}</span>
      {led ? <span className={cx("led", led !== "off" && led)} aria-label={led === "on" ? "running" : led === "wait" ? "waiting for you" : undefined} /> : null}
      {count ? <span className={cx("ct", countHot && "hot")}>{count}</span> : null}
      {progress != null ? (
        <span className="pg" aria-hidden>
          <i style={{ width: `${Math.max(0, Math.min(100, progress))}%` }} />
        </span>
      ) : null}
    </button>
  );
}

/** A module: a card with a graphite label band (icon, uppercase title, count). `band="hot"` is the screen's one orange band. */
export function Module({
  id,
  title,
  icon,
  count,
  band,
  tools,
  children,
  className,
  bodyClassName,
  style,
  auto,
}: {
  id?: string;
  title: string;
  icon?: IconName;
  count?: ReactNode;
  band?: "hot" | "bad";
  tools?: ReactNode;
  children?: ReactNode;
  className?: string;
  bodyClassName?: string;
  style?: CSSProperties;
  auto?: boolean;
}) {
  return (
    <section id={id} className={cx("panel", auto && "auto", className)} style={style} aria-label={title}>
      <header className={cx("ph", band)}>
        <span className="pt">
          {icon ? <Icon name={icon} /> : null}
          {title}
        </span>
        {count != null ? <span className="cnt">{count}</span> : null}
        <span className="sp" />
        {tools}
      </header>
      <div className={cx("pb", bodyClassName)}>{children}</div>
    </section>
  );
}

/** An LCD inlay. Numbers live here, in the dot-matrix font. Green normal, `hot` amber (soon), `bad` red (problem). */
export function Lcd({
  label,
  value,
  unit,
  sub,
  tone,
  bar,
  level,
  size,
  className,
}: {
  label: string;
  value: ReactNode;
  unit?: string;
  sub?: ReactNode;
  tone?: "hot" | "bad" | null;
  bar?: number | null;
  level?: { on: number; of: number } | null;
  size?: "sm" | "big";
  className?: string;
}) {
  return (
    <div className={cx("display", tone, size, className)} role="group" aria-label={label}>
      <span className="d-lbl">{label}</span>
      <span className="d-num">
        {value}
        {unit ? <small>{unit}</small> : null}
      </span>
      {sub != null ? <span className="d-sub">{sub}</span> : null}
      {bar != null ? (
        <span className="d-bar">
          <i style={{ width: `${Math.max(0, Math.min(1, bar)) * 100}%` }} />
        </span>
      ) : null}
      {level ? <Meter on={level.on} of={level.of} /> : null}
    </div>
  );
}

/** A level as LCD blocks (never a thin slider). */
export function Meter({ on, of, label }: { on: number; of: number; label?: string }) {
  const n = Math.max(1, of);
  const lit = Math.max(0, Math.min(n, Math.round(on)));
  return (
    <span className="meter" role="meter" aria-valuemin={0} aria-valuemax={n} aria-valuenow={lit} aria-label={label}>
      {Array.from({ length: n }, (_, i) => (
        <i key={i} className={i < lit ? "on" : undefined} />
      ))}
    </span>
  );
}

/** A filter chip with an optional count beside its label. */
export function Chip({ on, count, onClick, children, disabled, title }: { on?: boolean; count?: number | null; onClick?: () => void; children: ReactNode; disabled?: boolean; title?: string }) {
  return (
    <button type="button" className={cx("chip", on && "on")} aria-pressed={on} onClick={onClick} disabled={disabled} title={title}>
      {children}
      {count != null ? <span className="n">{count}</span> : null}
    </button>
  );
}

/** A segmented control. */
export function Seg<V extends string>({ options, value, onChange, label }: { options: { value: V; label: ReactNode; count?: number }[]; value: V; onChange: (v: V) => void; label: string }) {
  return (
    <div className="seg" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button key={o.value} type="button" role="radio" aria-checked={o.value === value} className={cx("chip", o.value === value && "on")} onClick={() => onChange(o.value)}>
          {o.label}
          {o.count != null ? <span className="n">{o.count}</span> : null}
        </button>
      ))}
    </div>
  );
}

/** A slide switch: a recessed slot and a raised knob. */
export function Switch({ checked, onChange, label, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return <button type="button" role="switch" aria-checked={checked} aria-label={label} className="sw" disabled={disabled} onClick={() => onChange(!checked)} />;
}

export function SwitchRow({ checked, onChange, children, disabled, hint }: { checked: boolean; onChange: (v: boolean) => void; children: ReactNode; disabled?: boolean; hint?: ReactNode }) {
  return (
    <label className="swrow">
      <span>
        {children}
        {hint ? <span className="hint">{hint}</span> : null}
      </span>
      <Switch checked={checked} onChange={onChange} label={typeof children === "string" ? children : "toggle"} disabled={disabled} />
    </label>
  );
}

/** A status pill. */
export function Pill({ tone, children }: { tone?: "soon" | "hot" | "over" | "live" | "ok"; children: ReactNode }) {
  return <span className={cx("duepill", tone)}>{children}</span>;
}

/** A coloured tag. `color` is any CSS colour, normally one of --k1..--k6. */
export function Tag({ color, children, title }: { color?: string; children: ReactNode; title?: string }) {
  return (
    <span className="cchip" style={color ? ({ "--kc": color } as CSSProperties) : undefined} title={title}>
      {children}
    </span>
  );
}

/** The search field, with active filters as chips inside it. */
export function SearchField({
  value,
  onChange,
  placeholder,
  chips,
  onSubmit,
  autoFocus,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
  chips?: { label: string; onRemove: () => void }[];
  onSubmit?: () => void;
  autoFocus?: boolean;
}) {
  return (
    <div className="fbox" role="search">
      <Icon name="browse" />
      {chips?.map((c) => (
        <button key={c.label} type="button" className="chip on" onClick={c.onRemove} title={`Remove ${c.label}`}>
          {c.label} <span className="x">×</span>
        </button>
      ))}
      <input
        className="finput"
        type="search"
        value={value}
        placeholder={placeholder}
        aria-label={placeholder}
        autoFocus={autoFocus}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && onSubmit?.()}
      />
    </div>
  );
}

/** A group header in a list. */
export function Grp({ children, count }: { children: ReactNode; count?: ReactNode }) {
  return (
    <div className="grp" role="separator">
      {children}
      {count != null ? <span>{count}</span> : null}
    </div>
  );
}

/** An empty state. */
export function Empty({ icon, title, children, actions }: { icon: IconName; title: string; children?: ReactNode; actions?: ReactNode }) {
  return (
    <div className="m-empty">
      <Icon name={icon} big />
      <b>{title}</b>
      {children ? <span>{children}</span> : null}
      {actions ? <div className="actions">{actions}</div> : null}
    </div>
  );
}

/** A small thumbnail, with a pixel placeholder when there is no image (or it fails to load). */
export function Thumb({ src, alt }: { src: string | null; alt: string }) {
  return (
    <span className="thumb">
      {src ? <img src={src} alt={alt} loading="lazy" onError={(e) => ((e.target as HTMLImageElement).style.display = "none")} /> : null}
      <Icon name="chip" />
    </span>
  );
}

/** A size as a small LCD chip: numbers live on LCDs. */
export function SizeChip({ bytes: n }: { bytes: number | null | undefined }) {
  const [num, unit] = bytesParts(n);
  return (
    <span className="lcd-chip m-size" title={`${num} ${unit}`}>
      <b>{num}</b>
      {unit}
    </span>
  );
}

export { cx };
