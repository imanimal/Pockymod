// For mod authors, in four tabs: My mods (your mods on Nexus, their files and pages), New mod (a guided trip to
// the website's form, which is the only way to make a page), Check and pack, and Publish a version (through the
// Upload API, picking the file from your mods instead of typing its ID).
import { useCallback, useEffect, useRef, useState } from "react";
import { Chip } from "../ui/pocket";
import { authoring, errorText, nexus, settings, type MyMods as MyModsData, type NexusStatus } from "../lib/api";
import { MyMods } from "./create/MyMods";
import { NewMod } from "./create/NewMod";
import { CheckModule, PackModule } from "./create/Pack";
import { Publish } from "./create/Publish";
import { usePack, type PublishPrefill } from "./create/shared";

type Tab = "mine" | "new" | "pack" | "publish";
const TABS: Array<{ value: Tab; label: string }> = [
  { value: "mine", label: "My mods" }, { value: "new", label: "New mod" }, { value: "pack", label: "Check and pack" }, { value: "publish", label: "Publish a version" },
];
const WHO_KEY = "creator.user";

export function Create() {
  const [tab, setTab] = useState<Tab>("mine");
  const pack = usePack();
  const [st, setSt] = useState<NexusStatus | null>(null);
  const [who, setWho] = useState("");
  const [mine, setMine] = useState<MyModsData | null>(null);
  // loading from the start: the first look-up begins before anything could be shown
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [prefill, setPrefill] = useState<PublishPrefill | null>(null);
  const seq = useRef(0);

  /** Loads a member's mods; only the latest call lands. Says what went wrong, or null. */
  const load = useCallback(async (user: string, refresh = false): Promise<string | null> => {
    const n = ++seq.current;
    setLoading(true); setError(null);
    try {
      const m = await authoring.myMods(user || null, refresh);
      if (n !== seq.current) return null;
      setMine(m); setWho(user);
      settings.set(WHO_KEY, user).catch(() => {});
      return null;
    } catch (e) {
      if (n === seq.current) setError(errorText(e));
      return errorText(e);
    } finally { if (n === seq.current) setLoading(false); }
  }, []);

  // signed in: your own mods; otherwise the name typed last time, read-only
  useEffect(() => {
    let live = true;
    (async () => {
      const s = await nexus.status().catch(() => null);
      if (!live) return;
      setSt(s);
      const last = (await settings.get(WHO_KEY).catch(() => null)) ?? "";
      if (!live) return;
      if (s?.user) await load("");
      else if (last) await load(last);
      else setLoading(false);
    })();
    return () => { live = false; };
  }, [load]);

  const hasKey = !!st?.hasKey;
  return (
    <div className="stack cr" style={{ maxWidth: "110em" }}>
      <div className="vtabs" role="tablist" aria-label="Create">
        {TABS.map((t) => <Chip key={t.value} on={tab === t.value} onClick={() => setTab(t.value)}>{t.label}</Chip>)}
      </div>
      {tab === "mine" && <MyMods mine={mine} loading={loading} error={error} me={st?.user ?? null} who={who} onLoad={(u, r) => void load(u, r)}
        onPublish={(p) => { setPrefill(p); setTab("publish"); }} onNew={() => setTab("new")} onChanged={() => void load(who, true)} />}
      {tab === "new" && <NewMod pack={pack} mine={mine} onOpenMine={() => setTab("mine")}
        onRefresh={async () => { const e = await load(who, true); if (e) throw new Error(e); }} />}
      {tab === "pack" && (
        <div className="grid2" style={{ alignItems: "start", maxWidth: "84em" }}>
          <CheckModule pack={pack} />
          <PackModule pack={pack} primary />
        </div>
      )}
      {tab === "publish" && <Publish pack={pack} mine={mine} hasKey={hasKey} prefill={prefill} onPack={() => setTab("pack")} onPublished={() => void load(who, true)} />}
    </div>
  );
}
