// Premium, said only where it saves something: a free account about to click through several Nexus pages sees what
// Premium would do instead. A quiet strip, never a dialog; Not now keeps it away for two weeks, and Settings turns it
// off. After Get Premium, coming back to the window checks the account again, so the queue goes on by itself.
import { useEffect, useState } from "react";
import { Key } from "./pocket";
import { toast } from "./toast";
import { errorText, nexus, nexusPages, settings } from "../lib/api";

/** The site's Premium page, as Nexus's help centre gives it (help.nexusmods.com, article 96). */
export const PREMIUM_PAGE = "https://next.nexusmods.com/premium";
export const PREMIUM_TIPS = "tips.premium";
const SNOOZE = "pm.premium.snooze";
/** One click is no bother; say nothing below this. */
const MIN_CLICKS = 2;

const read = (k: string) => { try { return Number(localStorage.getItem(k)) || 0; } catch { return 0; } };
const write = (k: string, v: number) => { try { localStorage.setItem(k, String(v)); } catch { /* storage unavailable */ } };

/** Open the Premium page where Settings says Nexus pages open, then look at the account again each time the window
 *  comes back (at most every 30 seconds, for an hour): once it is Premium, say so and let the app know. */
export function getPremium() {
  nexusPages.url(PREMIUM_PAGE).catch((e) => toast(errorText(e), true));
  const until = Date.now() + 3600_000;
  let last = 0;
  const h = () => {
    if (Date.now() > until) { window.removeEventListener("focus", h); return; }
    if (Date.now() - last < 30_000) return;
    last = Date.now();
    nexus.status(true).then((s) => {
      if (!s.user?.isPremium) return;
      window.removeEventListener("focus", h);
      window.dispatchEvent(new Event("pm-premium"));
      toast("Premium is on. Downloads run by themselves now, with no clicks on Nexus.");
    }).catch(() => {});
  };
  window.addEventListener("focus", h);
}

/** The strip, for `clicks` Nexus pages a free account is about to click through. Renders nothing for Premium,
 *  without an account, under two clicks, while snoozed or when turned off in Settings. */
export function PremiumNote({ clicks, what = "these" }: { clicks: number; what?: string }) {
  const [free, setFree] = useState(false);
  const [off, setOff] = useState(true);
  const [snoozed, setSnoozed] = useState(() => read(SNOOZE) > Date.now());
  useEffect(() => {
    const load = () => {
      nexus.status().then((s) => setFree(!!s.user && !s.user.isPremium)).catch(() => {});
      settings.get(PREMIUM_TIPS).then((v) => setOff(v === "0")).catch(() => setOff(false));
    };
    load();
    window.addEventListener("pm-prefs", load);
    window.addEventListener("pm-premium", load);
    return () => { window.removeEventListener("pm-prefs", load); window.removeEventListener("pm-premium", load); };
  }, []);
  if (!free || off || snoozed || clicks < MIN_CLICKS) return null;
  return (
    <div className="premium-strip" role="note">
      <span className="m-main"><b className="hi">Premium would download {what} {clicks} by itself.</b>
        <span className="dim"> No Nexus pages to click through and full download speed, and it helps pay for Nexus and the rewards it gives mod authors.</span></span>
      <Key size="sm" icon="web" onClick={getPremium} title="Nexus Premium, on the Nexus site. Come back here and the queue goes on by itself.">Get Premium</Key>
      <Key size="sm" variant="ghost" onClick={() => { write(SNOOZE, Date.now() + 14 * 86400_000); setSnoozed(true); }} title="Hide this for two weeks. Settings, Nexus account turns it off for good.">Not now</Key>
    </div>
  );
}
