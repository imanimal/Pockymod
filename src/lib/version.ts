// Version strings as mod authors write them.

/** The next version after this one: the last number goes up by one (1.28.6 → 1.28.7, v2 → v3). */
export function bump(v: string): string | null {
  const m = /^(.*?)(\d+)(\D*)$/.exec(v.trim());
  return m ? `${m[1]}${Number(m[2]) + 1}${m[3]}` : null;
}
