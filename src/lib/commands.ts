// Console commands in mod descriptions (typed in Cyber Engine Tweaks' console in game): Game.AddToInventory and the
// other Game.* calls. Found in the description text, de-duplicated, and kept in the order they appear. Bare item IDs
// ("Items.Foo") listed without a command become Game.AddToInventory lines, marked as made by the app.

export interface ConsoleCommand { command: string; generated: boolean }

const QUOTES: Array<[RegExp, string]> = [[/[“”„″]/g, '"'], [/[‘’′]/g, "'"]];

function normalise(s: string): string {
  let t = s;
  for (const [r, v] of QUOTES) t = t.replace(r, v);
  return t.replace(/ /g, " ");
}

/** A Game.* call: the name, then a bracketed argument list (one level of nested brackets allowed). */
const CALL = /\bGame\.[A-Za-z_][A-Za-z0-9_]*\s*\((?:[^()\n]|\([^()\n]*\))*\)\s*;?/g;
const ITEM_ID = /(["']?)\b(Items\.[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)*)\1/g;

export function extractCommands(text: string, limit = 200): ConsoleCommand[] {
  const t = normalise(text);
  const out: ConsoleCommand[] = [];
  const seen = new Set<string>();
  const covered: Array<[number, number]> = [];
  for (const m of t.matchAll(CALL)) {
    const cmd = m[0].trim().replace(/\s+\(/, "(").replace(/;$/, "");
    covered.push([m.index ?? 0, (m.index ?? 0) + m[0].length]);
    if (!seen.has(cmd)) { seen.add(cmd); out.push({ command: cmd, generated: false }); }
    if (out.length >= limit) return out;
  }
  // item IDs outside any command: offer the usual "give me this item" line
  for (const m of t.matchAll(ITEM_ID)) {
    const at = m.index ?? 0;
    if (covered.some(([a, b]) => at >= a && at < b)) continue;
    const id = m[2];
    const already = out.some((c) => c.command.includes(`"${id}"`) || c.command.includes(`'${id}'`));
    const cmd = `Game.AddToInventory("${id}", 1)`;
    if (!already && !seen.has(cmd)) { seen.add(cmd); out.push({ command: cmd, generated: true }); }
    if (out.length >= limit) break;
  }
  return out;
}
