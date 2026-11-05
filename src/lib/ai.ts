// The AI labels Nexus puts on mods, and the Browse setting that hides them. The three names are Nexus's own tags, read
// from the live facets for Cyberpunk 2077 on 2026-11-05 (AI-Generated Content 1,151 mods, AI Media 606, AI Assisted 326).
// "NPC Behaviours (AI)" is another tag, about the game's own AI, and is not one of these.

export type AiKind = "generated" | "media" | "assisted";
/** What Browse leaves out: nothing, mods Nexus tags AI-Generated Content, or every AI-tagged mod. */
export type AiHide = "show" | "generated" | "all";

export const AI_TAGS: Record<AiKind, string> = { generated: "AI-Generated Content", media: "AI Media", assisted: "AI Assisted" };
/** Shown on cards and pages. The tips say what Nexus means by each tag. */
export const AI_INFO: Record<AiKind, { label: string; tip: string }> = {
  generated: { label: "AI-generated", tip: "Nexus tag: the author says AI made much of this mod's code or assets. Its Nexus page has the details" },
  media: { label: "AI media", tip: "Nexus tag: AI made this mod's page art, thumbnails or description, not the mod itself" },
  assisted: { label: "AI assisted", tip: "Nexus tag: a person led the work and AI helped. The author names how on the mod's Nexus page" },
};
export const AI_OPTIONS: Array<{ value: AiHide; label: string }> = [
  { value: "show", label: "Show" }, { value: "generated", label: "Hide AI-generated" }, { value: "all", label: "Hide all AI" },
];

const ORDER: AiKind[] = ["generated", "media", "assisted"];

/** The AI labels on a mod, most to least AI. */
export function aiKinds(tags: ReadonlyArray<{ name: string }> | undefined): AiKind[] {
  const names = new Set((tags ?? []).map((t) => t.name.trim().toLowerCase()));
  return ORDER.filter((k) => names.has(AI_TAGS[k].toLowerCase()));
}

/** The tags a setting leaves out. */
export function aiExcluded(hide: AiHide): string[] {
  return hide === "all" ? ORDER.map((k) => AI_TAGS[k]) : hide === "generated" ? [AI_TAGS.generated] : [];
}

/** A search with the setting applied. A tag the user asked for by name stays: picking "AI Assisted" shows them. */
export function withAiHidden<Q extends { tags: string[]; excludeTags: string[] }>(q: Q, hide: AiHide): Q {
  const add = aiExcluded(hide).filter((t) => !q.tags.includes(t) && !q.excludeTags.includes(t));
  return add.length ? { ...q, excludeTags: [...q.excludeTags, ...add] } : q;
}

export const aiHideText = (hide: AiHide) => (hide === "generated" ? "Hiding AI-generated" : hide === "all" ? "Hiding all AI" : "");
