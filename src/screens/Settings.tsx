// Settings: the Nexus key (kept in Windows Credential Manager), the game folder, the nxm:// handler, how files
// are placed, appearance, update checks, and experiments.

import { useEffect, useState } from "react";
import { Deck } from "../components/Deck";
import { Key, Lcd, Module, Pill, Seg, SwitchRow } from "../components/pocket";
import { api, pickFolder } from "../lib/api";
import { count, date } from "../lib/format";
import { applyTheme, useAction, useApp } from "../lib/store";
import type { HandlerStatus, Release, Settings as S } from "../lib/types";

export function Settings() {
  const { status, refresh, toast, go } = useApp();
  const [s, setS] = useState<S | null>(null);
  const [key, setKey] = useState("");
  const [handler, setHandler] = useState<HandlerStatus | null>(null);
  const [release, setRelease] = useState<Release | null | undefined>(undefined);
  const [modioKey, setModioKey] = useState("");
  const { busy, run } = useAction();

  useEffect(() => {
    api.settings().then(setS);
    api.nxmStatus().then(setHandler).catch(() => {});
  }, []);

  const save = async (next: S) => {
    setS(next);
    try {
      setS(await api.saveSettings(next));
      applyTheme(next.theme);
      document.documentElement.dataset.motion = next.motion ? "1" : "0";
      refresh();
    } catch (e) {
      toast(String((e as { message?: string }).message ?? e), { tone: "bad" });
    }
  };

  if (!s || !status) return null;
  const acct = status.account;
  const r = status.rate;
  const game = status.game;

  return (
    <div className="settings">
      <Deck>
        <Key icon="grid" onClick={() => go("gallery")}>
          Component gallery
        </Key>
        <Key icon="refresh" onClick={() => { api.nxmStatus().then(setHandler); refresh(); }}>
          Refresh
        </Key>
      </Deck>

      <Module title="Nexus account" icon="key" count={acct ? acct.name : "not connected"} auto>
        <div className="lcds">
          <Lcd label="Account" value={acct ? (acct.isPremium ? "PREMIUM" : "FREE") : "--"} sub={acct ? acct.name : "No key yet"} size="sm" />
          <Lcd label="Today" value={r.dailyRemaining != null ? count(r.dailyRemaining) : "--"} sub={`of ${count(r.dailyLimit ?? 20000)} requests`} tone={r.level === "low" && (r.dailyRemaining ?? 1) > 0 ? "hot" : null} size="sm" bar={r.dailyRemaining != null && r.dailyLimit ? r.dailyRemaining / r.dailyLimit : null} />
          <Lcd label="This hour" value={r.hourlyRemaining != null ? count(r.hourlyRemaining) : "--"} sub="after the daily cap" tone={r.level === "throttled" ? "bad" : r.level === "low" && r.dailyRemaining === 0 ? "hot" : null} size="sm" />
        </div>
        <div className="fgrid">
          <label htmlFor="key">Personal API key</label>
          <input id="key" type="password" autoComplete="off" spellCheck={false} placeholder={status.hasKey ? "A key is saved. Paste a new one to replace it." : "Paste the key from your Nexus account page"} value={key} onChange={(e) => setKey(e.target.value)} />
        </div>
        <div className="actions">
          <Key
            variant={status.hasKey ? undefined : "primary"}
            icon="check"
            disabled={!key.trim() || busy === "key"}
            onClick={() =>
              run("key", async () => {
                const a = await api.setApiKey(key);
                setKey("");
                toast(`Connected as ${a.name} (${a.isPremium ? "Premium" : "free account"}).`);
                refresh();
              })
            }
          >
            {busy === "key" ? "Checking…" : "Check and save"}
          </Key>
          {status.hasKey ? (
            <>
              <Key icon="refresh" onClick={() => run("acct", async () => { await api.refreshAccount(); refresh(); })}>
                Refresh account
              </Key>
              <Key icon="info" onClick={() => run("probe", async () => { await api.probe(); toast("Live check done: the response shape was saved as nexus-probe.json in the app's data folder."); refresh(); })}>
                Live API check
              </Key>
              <Key variant="ghost" icon="trash" onClick={() => run("forget", async () => { await api.clearApiKey(); refresh(); })}>
                Forget key
              </Key>
            </>
          ) : null}
        </div>
        <p className="note">
          Your own personal key, for your own use. It's checked with Nexus, then stored in Windows Credential Manager, never in a file or a log. Find it on
          nexusmods.com under your account's API Keys page.
        </p>
      </Module>

      <Module title="Game" icon="play" count={game ? game.launcher : "not found"} auto band={game ? undefined : "bad"}>
        {game ? (
          <div className="lcds">
            <Lcd label="Version" value={game.version ?? "?"} sub={game.buildTime ? `build ${date(game.buildTime)}` : "unknown build"} size="sm" />
            <Lcd label="REDmod" value={game.redmodTool ? "YES" : "NO"} tone={game.redmodTool ? null : "hot"} sub={game.redmodTool ? "tool installed" : "DLC not installed"} size="sm" />
          </div>
        ) : null}
        <p className="path mono">{game?.root ?? "Pocket Mods couldn't find Cyberpunk 2077."}</p>
        {game?.underProgramFiles ? <p className="note warnc">The game is under Program Files, where Windows can block writes. If installs fail, move it to another library folder.</p> : null}
        <div className="actions">
          <Key icon="folder" onClick={() => run("pick", async () => { const p = await pickFolder("Pick the Cyberpunk 2077 folder"); if (p) { await api.setGameRoot(p); refresh(); } })}>
            Pick the game folder
          </Key>
          <Key icon="refresh" onClick={() => run("detect", async () => { const g = await api.detectGames(); toast(g.length ? `Found ${g.map((x) => `${x.launcher}: ${x.root}`).join("; ")}` : "No install found. Pick the folder by hand."); })}>
            Detect again
          </Key>
        </div>
      </Module>

      <Module title="nxm:// links" icon="link" count={handler?.ours ? "Pocket Mods" : handler?.holder ?? "nobody"} auto>
        <p className="note">
          Nexus's “Mod Manager Download” button opens an nxm:// link. Pocket Mods registers for it per user (no admin rights). Links clicked inside the app are caught
          either way.
        </p>
        {handler && !handler.supported ? <Pill>Windows only</Pill> : null}
        {handler?.supported ? (
          <div className="actions">
            {handler.ours ? <Pill tone="ok">Pocket Mods handles nxm:// links</Pill> : <Pill tone="soon">{handler.holder ? `${handler.holder} handles them now` : "Nobody handles them"}</Pill>}
            {!handler.ours ? (
              <Key icon="link" onClick={() => run("nxm", async () => setHandler(await api.nxmRegister()))}>
                {handler.holder ? `Take back from ${handler.holder}` : "Handle nxm:// links"}
              </Key>
            ) : handler.previous ? (
              <Key variant="ghost" icon="skip" onClick={() => run("nxm", async () => setHandler(await api.nxmRestore()))}>
                Hand back to the previous manager
              </Key>
            ) : null}
          </div>
        ) : null}
      </Module>

      <Module title="Installing" icon="folder" auto>
        <p className="path mono">Mod store: {status.storeRoot ?? "set once the game is found"}</p>
        <p className="note">The store sits on the game's drive so files can be hard-linked into the game folder: no copies, no admin rights, and every install can be undone.</p>
        <SwitchRow checked={s.allowCopy} onChange={(v) => save({ ...s, allowCopy: v })} hint={s.allowCopy ? "Copies use disk space twice and are slower to undo." : "Off: an install stops with an explanation instead."}>
          Copy when hard links fail
        </SwitchRow>
        <SwitchRow checked={s.laterWins} onChange={(v) => save({ ...s, laterWins: v })} hint="When two mods write the same file, the one installed or enabled last wins. Off: the first keeps it.">
          Newer installs win conflicts
        </SwitchRow>
        <SwitchRow checked={s.redmodAutoDeploy} onChange={(v) => save({ ...s, redmodAutoDeploy: v })} hint="Runs the game's REDmod deploy step after installing a REDmod.">
          Deploy REDmods automatically
        </SwitchRow>
        <SwitchRow checked={s.includeAdult} onChange={(v) => save({ ...s, includeAdult: v })}>
          Show adult mods in Browse
        </SwitchRow>
        <div className="actions">
          <Key icon="folder" onClick={() => run("store", async () => { const p = await pickFolder("Pick a folder for the mod store (same drive as the game)"); if (p) await save({ ...s, storeRoot: p }); })}>
            Move the store…
          </Key>
        </div>
      </Module>

      <Module title="Appearance" icon="sun" auto>
        <Seg
          label="Theme"
          value={s.theme}
          onChange={(v) => save({ ...s, theme: v })}
          options={[
            { value: "system", label: "Like Windows" },
            { value: "cream", label: "Cream" },
            { value: "night", label: "Night" },
          ]}
        />
        <SwitchRow checked={s.motion} onChange={(v) => save({ ...s, motion: v })}>
          Key travel and LCD flicker
        </SwitchRow>
      </Module>

      <Module title="Updates" icon="refresh" count={`v${status.version}`} auto>
        <SwitchRow checked={s.checkAppUpdates} onChange={(v) => save({ ...s, checkAppUpdates: v })}>
          Check for new versions of Pocket Mods
        </SwitchRow>
        <div className="actions">
          <Key icon="refresh" disabled={busy === "upd"} onClick={() => run("upd", async () => setRelease(await api.appUpdateCheck()))}>
            Check now
          </Key>
          {release === null ? <Pill tone="ok">No release published yet</Pill> : release ? release.newer ? <Pill tone="soon">Version {release.version} is out</Pill> : <Pill tone="ok">Up to date</Pill> : null}
        </div>
        {release?.newer ? <p className="note mono">{release.url}</p> : null}
        <p className="note">The installer isn't code-signed yet, so Windows SmartScreen may warn the first time you run it.</p>
      </Module>

      <Module title="Experiments" icon="bolt" auto>
        <SwitchRow checked={s.experiments.publishCollections} onChange={(v) => save({ ...s, experiments: { ...s.experiments, publishCollections: v } })} hint="Unconfirmed that a third-party app may publish collections to Nexus.">
          Publish collections to Nexus
        </SwitchRow>
        <SwitchRow checked={s.experiments.modio} onChange={(v) => save({ ...s, experiments: { ...s.experiments, modio: v } })} hint="Read-only browsing of mod.io; the game installs those itself.">
          mod.io browsing
        </SwitchRow>
        {s.experiments.modio ? (
          <div className="fgrid">
            <label htmlFor="mk">mod.io API key</label>
            <span className="inline-form">
              <input id="mk" type="password" value={modioKey} onChange={(e) => setModioKey(e.target.value)} />
              <Key size="sm" disabled={!modioKey.trim()} onClick={() => run("mk", async () => { await api.modioSetKey(modioKey); setModioKey(""); toast("mod.io key saved in Credential Manager."); })}>
                Save
              </Key>
            </span>
            <label htmlFor="mg">Cyberpunk's mod.io game ID</label>
            <input id="mg" type="number" value={s.modioGameId ?? ""} onChange={(e) => save({ ...s, modioGameId: e.target.value ? Number(e.target.value) : null })} />
          </div>
        ) : null}
      </Module>
    </div>
  );
}
