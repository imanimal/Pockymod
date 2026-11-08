// Sign in with Nexus (lib/sso.ts): one key, then approve on the Nexus site. Shows nothing until Nexus has issued
// the app's slug; the personal-key field stays beside it for tinkerers.
import { useEffect, useRef, useState } from "react";
import { Key } from "./pocket";
import { toast } from "./toast";
import { errorText, type NexusStatus } from "../lib/api";
import { signIn, ssoReady } from "../lib/sso";

export function NexusSignIn({ onDone, primary }: { onDone: (s: NexusStatus) => void; primary?: boolean }) {
  const [waiting, setWaiting] = useState(false);
  const cancel = useRef<(() => void) | null>(null);
  useEffect(() => () => cancel.current?.(), []);
  if (!ssoReady()) return null;
  const go = () => {
    const s = signIn(() => toast("Approve Pockymod on Nexus in your browser"));
    cancel.current = s.cancel;
    setWaiting(true);
    s.done.then((st) => { toast(`Signed in as ${st.user?.name ?? "your Nexus account"}`); onDone(st); })
      .catch((e) => { if ((e as { kind?: string })?.kind !== "cancelled") toast(errorText(e), true); })
      .finally(() => { setWaiting(false); cancel.current = null; });
  };
  return (
    <div className="actions">
      <Key variant={primary ? "primary" : undefined} icon="key" busy={waiting} onClick={go}
        title="Approve Pockymod on Nexus in your browser">{waiting ? "Waiting for Nexus" : "Sign in with Nexus"}</Key>
      {waiting && <Key variant="ghost" onClick={() => cancel.current?.()} title="Stop waiting">Cancel</Key>}
    </div>
  );
}
