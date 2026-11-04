// Typed calls to the Rust side. Shapes mirror pocket-core's Serialize output (camelCase).
import { call, listen } from "./tauri";

export interface ApiError { kind: string; message: string }
export const errorText = (e: unknown): string =>
  typeof e === "object" && e && "message" in e ? String((e as ApiError).message) : String(e);
export const errorKind = (e: unknown): string =>
  typeof e === "object" && e && "kind" in e ? String((e as ApiError).kind) : "unknown";

export type RateLevel = "unknown" | "ok" | "low" | "throttled";
export interface RateDisplay {
  level: RateLevel;
  remaining: number | null;
  limit: number | null;
  window: "hour" | "day";
  resetsAt: string | null;
  throttledUntil: string | null;
}

export interface NexusUser { userId: number; name: string; isPremium: boolean; isSupporter: boolean; profileUrl?: string | null }
export interface NexusStatus { hasKey: boolean; user: NexusUser | null; rateLimit: RateDisplay }

export const nexus = {
  status: (refresh = false) => call<NexusStatus>("nexus_status", { refresh }),
  setKey: (key: string) => call<NexusStatus>("nexus_set_key", { key }),
  clearKey: () => call<NexusStatus>("nexus_clear_key"),
  rateLimit: () => call<RateDisplay>("rate_limit"),
  onRateLimit: (cb: (r: RateDisplay) => void) => listen<RateDisplay>("rate-limit", cb),
};
