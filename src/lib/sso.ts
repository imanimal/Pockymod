// Nexus single sign-on: the user approves Pockymod on the Nexus site, where they are already signed in, and the key
// comes back over Nexus's SSO socket, so nobody copies a key by hand. Nexus gives a registered app its slug; until
// then SSO_SLUG is empty, no sign-in key shows, and the app asks for a personal key instead.
// UNVERIFIED: the messages follow Nexus's sso-integration-demo (protocol 2); not yet run against the live service.
import { nexus, openExternal, type NexusStatus } from "./api";

/** The application slug Nexus issues on registration. */
export const SSO_SLUG = "";
export const ssoReady = () => SSO_SLUG !== "";
const SOCKET = "wss://sso.nexusmods.com";
/** The approval page waits this long before the attempt gives up. */
const GIVE_UP = 10 * 60_000;

/** Open the approval page and wait for the key, then check and save it like a pasted one (Credential Manager). */
export function signIn(onWaiting?: () => void): { done: Promise<NexusStatus>; cancel: () => void } {
  const id = crypto.randomUUID();
  let ws: WebSocket | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let stop: (e: unknown) => void = () => {};
  const done = new Promise<NexusStatus>((resolve, reject) => {
    const end = (e: unknown) => { clearTimeout(timer); ws?.close(); reject(e); };
    stop = end;
    ws = new WebSocket(SOCKET);
    timer = setTimeout(() => end({ kind: "invalid", message: "No answer from Nexus. Try signing in again" }), GIVE_UP);
    ws.onopen = () => {
      ws?.send(JSON.stringify({ id, token: null, protocol: 2 }));
      openExternal(`https://www.nexusmods.com/sso?id=${id}&application=${SSO_SLUG}`).then(() => onWaiting?.()).catch(end);
    };
    ws.onmessage = (ev) => {
      let m: { success?: boolean; error?: string; data?: { connection_token?: string; api_key?: string } };
      try { m = JSON.parse(String(ev.data)); } catch { return; }
      if (!m.success) { end({ kind: "invalid", message: m.error || "Nexus sign-in failed" }); return; }
      if (m.data?.api_key) { clearTimeout(timer); ws?.close(); nexus.setKey(m.data.api_key).then(resolve, reject); }
      // a connection_token only matters for reconnecting, which a fresh attempt replaces
    };
    ws.onerror = () => end({ kind: "network", message: "Couldn't reach Nexus sign-in. Check your connection" });
  });
  return { done, cancel: () => stop({ kind: "cancelled", message: "Sign-in cancelled." }) };
}
