// Demo backend for running the UI in a plain browser (npm run dev). It answers the same commands as the Rust side
// with fixed data, so screens can be built and checked without Tauri. Nothing here talks to Nexus.
import { demoEmit, inTauri, registerDemo } from "./tauri";
import type { NexusStatus, RateDisplay } from "./api";

const rate: RateDisplay = { level: "unknown", remaining: null, limit: null, window: "hour", resetsAt: null, throttledUntil: null };
let status: NexusStatus = { hasKey: false, user: null, rateLimit: rate };

export function installDemo() {
  if (inTauri()) return;
  registerDemo({
    nexus_status: () => status,
    rate_limit: () => status.rateLimit,
    nexus_set_key: ({ key }) => {
      if (String(key).trim().length < 16) throw { kind: "invalid", message: "That does not look like a Nexus API key." };
      status = {
        hasKey: true,
        user: { userId: 1, name: "demo-user", isPremium: false, isSupporter: false },
        rateLimit: { level: "ok", remaining: 487, limit: 500, window: "hour", resetsAt: null, throttledUntil: null },
      };
      demoEmit("rate-limit", status.rateLimit);
      return status;
    },
    nexus_clear_key: () => { status = { hasKey: false, user: null, rateLimit: rate }; return status; },
  });
}
