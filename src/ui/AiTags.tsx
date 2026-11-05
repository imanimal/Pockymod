// The AI labels Nexus puts on a mod, as small tags on its card and page (lib/ai.ts).
import { Tag } from "./pocket";
import { AI_INFO, aiKinds } from "../lib/ai";

export function AiTags({ tags }: { tags?: ReadonlyArray<{ name: string }> }) {
  return <>{aiKinds(tags).map((k) => <span key={k} className="ai-tag" title={AI_INFO[k].tip}><Tag square>{AI_INFO[k].label}</Tag></span>)}</>;
}
