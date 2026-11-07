// The browser pane's Nexus sign-in, copied from Firefox in one click (src-tauri/src/browser.rs). The notice says what
// is read and where it goes; pressing the key is the permission. Chrome and Edge lock their sign-in to themselves,
// so for them the pane offers to save the password the first time it's typed.
import { Key } from "./pocket";
import { toast } from "./toast";
import { browserPane, errorText, type PaneSignIn } from "../lib/api";

/** Said wherever the copy is offered. */
export const FIREFOX_NOTICE = "Copies only your Nexus sign-in cookies from Firefox into the app's browser. They stay on this PC and only go to nexusmods.com. Sign out in Settings, Nexus pages removes them.";
/** Fired after the pane's sign-in changes, so whatever shows it asks again. */
export const SIGN_IN_CHANGED = "pm-pane-signin";

export async function copyFirefoxSignIn(): Promise<PaneSignIn | null> {
  try {
    const s = await browserPane.useFirefox();
    toast(s.signedIn ? "Signed in to Nexus with your Firefox sign-in" : "Nexus didn't take the Firefox sign-in. Sign in on the page instead", !s.signedIn);
    window.dispatchEvent(new Event(SIGN_IN_CHANGED));
    return s;
  } catch (e) { toast(errorText(e), true); return null; }
}

export function UseFirefoxKey({ size, onDone }: { size?: "sm"; onDone?: (s: PaneSignIn) => void }) {
  return (
    <Key size={size} variant="primary" icon="key" title={FIREFOX_NOTICE}
      onClick={() => void copyFirefoxSignIn().then((s) => s && onDone?.(s))}>Use Firefox sign-in</Key>
  );
}
