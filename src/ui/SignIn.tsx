// Sign in with Nexus: Nexus's approval page opens in the browser and the app waits for it. Shows nothing until Nexus
// has issued the app's client ID; the personal-key field stays nearby for tinkerers.
import { useEffect, useRef, useState } from "react";
import { Key } from "./pocket";
import { toast } from "./toast";
import { errorKind, errorText, nexus, type NexusStatus } from "../lib/api";

export function NexusSignIn({ status, onDone, primary }: { status: NexusStatus | null; onDone: (s: NexusStatus) => void; primary?: boolean }) {
  const [waiting, setWaiting] = useState(false);
  const live = useRef(true);
  const waitingRef = useRef(false);
  // leaving the screen stops the wait, so a sign-in never finishes where nobody sees it
  useEffect(() => { live.current = true; return () => { live.current = false; if (waitingRef.current) void nexus.cancelSignIn(); }; }, []);
  if (!status?.signInAvailable) return null;
  const go = () => {
    waitingRef.current = true;
    setWaiting(true);
    toast("Approve Pockymod on Nexus in your browser");
    nexus.signIn()
      .then((st) => { toast(`Signed in as ${st.user?.name ?? "your Nexus account"}`); if (live.current) onDone(st); })
      .catch((e) => { if (errorKind(e) !== "cancelled") toast(errorText(e), true); })
      .finally(() => { waitingRef.current = false; if (live.current) setWaiting(false); });
  };
  return (
    <div className="actions">
      <Key variant={primary ? "primary" : undefined} icon="key" busy={waiting} disabled={waiting} onClick={go}
        title="Opens Nexus in your browser. Approve Pockymod there and you're signed in">{waiting ? "Waiting for Nexus" : "Sign in with Nexus"}</Key>
      {waiting && <Key variant="ghost" onClick={() => void nexus.cancelSignIn()} title="Stop waiting for the browser">Cancel</Key>}
    </div>
  );
}
