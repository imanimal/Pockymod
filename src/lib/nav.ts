// Going somewhere in the app from anywhere: a toast's link, a note's link. The window listens for "pm-go" (App.tsx).

/** The sections of Settings, by their titles on the screen. */
export const SETTINGS_SECTIONS = ["Nexus account", "Nexus pages", "Background", "Game and mod folders", "Installing", "Appearance", "Adult content", "AI content", "Experiments", "About"] as const;

export interface Go { view: string; section?: string }
export const go = (to: Go): void => { window.dispatchEvent(new CustomEvent<Go>("pm-go", { detail: to })); };
export const goSettings = (section?: string): void => go({ view: "settings", section });

/** Where a message sends the user ("... in Settings, Nexus pages"), so its toast can offer a link there. A message that
 *  only says "in Settings" is placed by what it is about; a mod's name that has the word in it is not a direction. */
export function settingsTarget(text: string): { section?: string } | null {
  const m = /\b(?:in|to|from|under|at) Settings\b/.exec(text);
  if (!m) return null;
  const rest = text.slice(m.index + m[0].length);
  const named = SETTINGS_SECTIONS.find((s) => rest.startsWith(`, ${s}`));
  if (named) return { section: named };
  if (/API key/i.test(text)) return { section: "Nexus account" };
  if (/game folder|Cyberpunk 2077 folder|mod library/i.test(text)) return { section: "Game and mod folders" };
  return {};
}
