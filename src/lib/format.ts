// Formatting for LCDs and rows. Numbers on LCDs stay short: 12.4K, 3.2 GB.

export function bytes(n: number | null | undefined): string {
  if (n == null || !isFinite(n) || n < 0) return "–";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${i === 0 ? v : v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

/** Splits a byte count into the LCD number and its unit. */
export function bytesParts(n: number | null | undefined): [string, string] {
  const s = bytes(n);
  const i = s.lastIndexOf(" ");
  return i < 0 ? [s, ""] : [s.slice(0, i), s.slice(i + 1)];
}

export function count(n: number | null | undefined): string {
  if (n == null || !isFinite(n)) return "–";
  if (n < 1000) return String(n);
  if (n < 10_000) return `${(n / 1000).toFixed(1)}K`;
  if (n < 1_000_000) return `${Math.round(n / 1000)}K`;
  if (n < 10_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  return `${Math.round(n / 1_000_000)}M`;
}

/** "3 Nov 2026" from Unix seconds. */
export function date(ts: number | null | undefined): string {
  if (!ts) return "–";
  return new Date(ts * 1000).toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" });
}

/** "today", "3 days ago", "2 months ago". */
export function ago(ts: number | null | undefined, now = Date.now() / 1000): string {
  if (!ts) return "–";
  const d = Math.floor((now - ts) / 86400);
  if (d < 1) return "today";
  if (d === 1) return "yesterday";
  if (d < 30) return `${d} days ago`;
  const m = Math.floor(d / 30);
  if (m < 12) return m === 1 ? "a month ago" : `${m} months ago`;
  const y = Math.floor(d / 365);
  return y <= 1 ? "a year ago" : `${y} years ago`;
}

/** Seconds as "2:05" or "1:02:05"; null as "–". */
export function duration(secs: number | null | undefined): string {
  if (secs == null || !isFinite(secs)) return "–";
  const s = Math.max(0, Math.round(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  const pad = (x: number) => String(x).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(r)}` : `${m}:${pad(r)}`;
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** The message from anything a command rejected with. */
export function errMsg(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  if (typeof e === "string") return e;
  return "Something went wrong.";
}
