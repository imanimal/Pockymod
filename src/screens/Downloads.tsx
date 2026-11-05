// Downloads: LCDs for mods left, clicks needed and time left (amber while the queue waits on you), then the
// queue. "Download next" is the screen's orange key: it opens the next mod's Nexus page inside the app.

import { useEffect, useState } from "react";
import { Deck } from "../components/Deck";
import { Empty, Grp, Key, Lcd, Meter, Module, Pill } from "../components/pocket";
import { api, on } from "../lib/api";
import { bytes, bytesParts, duration, plural } from "../lib/format";
import { useAction, useApp } from "../lib/store";
import type { ItemState, QueueItem, QueueView } from "../lib/types";

const STATE_LABEL: Record<ItemState, string> = {
  waiting: "Waiting",
  needsClick: "Needs your click",
  resolving: "Getting link",
  downloading: "Downloading",
  installing: "Installing",
  done: "Installed",
  failed: "Failed",
};

function statePill(s: ItemState) {
  const tone = s === "needsClick" ? "soon" : s === "failed" ? "over" : s === "done" ? "ok" : s === "downloading" || s === "installing" || s === "resolving" ? "live" : undefined;
  return <Pill tone={tone}>{STATE_LABEL[s]}</Pill>;
}

export function Downloads() {
  const { status, go } = useApp();
  const [v, setV] = useState<QueueView | null>(null);
  const [link, setLink] = useState("");
  const { busy, run } = useAction();

  useEffect(() => {
    api.queue().then(setV).catch(() => {});
    return on("queue", (p) => setV(p as QueueView));
  }, []);

  if (!v) return null;
  const s = v.summary;
  const pending = v.items.filter((i) => i.state === "waiting" || i.state === "needsClick");
  const next = v.items.find((i) => i.state === "needsClick") ?? pending[0];
  const active = v.items.filter((i) => ["resolving", "downloading", "installing"].includes(i.state));
  const finished = v.items.filter((i) => i.state === "done" || i.state === "failed");
  const [leftNum, leftUnit] = bytesParts(s.bytesLeft);
  const wait = s.waitingOnUser;

  return (
    <div className="downloads">
      <Deck>
        <Key icon={v.paused ? "play" : "pause"} onClick={() => run("pause", () => api.queueAction(v.paused ? "resume" : "pause"))}>
          {v.paused ? "Resume" : "Pause"}
        </Key>
        <Key icon="check" disabled={finished.length === 0} onClick={() => run("clear", () => api.queueAction("clear"))}>
          Clear finished
        </Key>
        <Key icon="browse" onClick={() => go("browse")}>
          Find more mods
        </Key>
      </Deck>

      <Module title="Queue status" icon="download" count={v.premium ? "Premium" : "Free account"} auto className="dl-status">
        <div className="lcds">
          <Lcd label="Mods left" value={s.left} sub={s.done ? `${s.done} installed` : "in the queue"} level={{ on: Math.round((s.percent / 100) * 12), of: 12 }} />
          <Lcd label="Needs your click" value={s.needsClick} tone={wait ? "hot" : null} sub={wait ? "Waiting for you on Nexus" : s.needsClick ? "Clicks on Nexus" : "Nothing to click"} />
          <Lcd label="Time left" value={s.etaSecs != null ? duration(s.etaSecs) : "--:--"} sub={`${leftNum} ${leftUnit} to download`} tone={s.failed ? "bad" : null} />
        </div>
        <div className="actions">
          <Key
            variant="primary"
            size="lg"
            icon="download"
            disabled={!next || busy === "next"}
            onClick={() => next && run("next", () => api.queueAction("open", next.id))}
          >
            Download next
          </Key>
          <span className="note">
            {v.premium
              ? "Premium downloads run by themselves. Manual-only mods still open their page."
              : s.needsClick
                ? `${plural(s.needsClick, "mod needs", "mods need")} a click on Nexus. The page opens inside the app; click “Mod Manager Download” and the queue moves on by itself.`
                : "Queued mods show here."}
          </span>
        </div>
      </Module>

      <Module title="Queue" icon="order" count={v.items.length || null} className="dl-queue" bodyClassName="list-body">
        {v.items.length === 0 ? (
          <Empty icon="download" title="The queue is empty" actions={<Key icon="browse" onClick={() => go("browse")}>Browse mods</Key>}>
            Install a mod from Browse, a collection, or Library's updates.
          </Empty>
        ) : (
          <div className="rows">
            {active.length ? <Grp count={active.length}>Now</Grp> : null}
            {active.map((i) => <QRow key={i.id} i={i} />)}
            {pending.length ? <Grp count={pending.length}>Up next</Grp> : null}
            {pending.map((i) => <QRow key={i.id} i={i} />)}
            {finished.length ? <Grp count={finished.length}>Finished</Grp> : null}
            {finished.map((i) => <QRow key={i.id} i={i} />)}
          </div>
        )}
        <div className="paste">
          <label htmlFor="nxm">Have an nxm:// link? Paste it here.</label>
          <input id="nxm" value={link} placeholder="nxm://cyberpunk2077/mods/…/files/…" onChange={(e) => setLink(e.target.value)} />
          <Key size="sm" disabled={!link.startsWith("nxm://")} onClick={() => run("nxm", async () => { await api.handleNxm(link.trim()); setLink(""); })}>
            Use link
          </Key>
        </div>
      </Module>
      {status && !status.hasKey ? <p className="note">Add your API key in Settings to download.</p> : null}
    </div>
  );
}

function QRow({ i }: { i: QueueItem }) {
  const { run } = useAction();
  const pct = i.sizeBytes ? i.received / i.sizeBytes : 0;
  return (
    <div className="row mrow qrow">
      <span className="qpos">{i.origin === "dependency" ? "dep" : i.origin.startsWith("collection") ? "col" : i.origin === "update" ? "upd" : ""}</span>
      <div className="m-main">
        <span className="m-t">{i.modName}</span>
        <span className="m-meta">
          <span>{i.fileName || "file"}</span>
          {i.version ? <span>v{i.version}</span> : null}
          <span>{bytes(i.sizeBytes)}</span>
          {i.error ? <span className="badc">{i.error}</span> : null}
        </span>
        {i.state === "downloading" ? <Meter on={pct * 20} of={20} label="Download progress" /> : null}
      </div>
      <span />
      <div className="m-due">
        {statePill(i.state)}
        <span className="actions sm">
          {i.state === "needsClick" ? (
            <Key size="sm" icon="link" onClick={() => run("open", () => api.queueAction("open", i.id))}>
              Open page
            </Key>
          ) : null}
          {i.state === "needsClick" || i.state === "waiting" ? (
            <Key size="sm" variant="ghost" icon="skip" onClick={() => run("skip", () => api.queueAction("skip", i.id))}>
              Later
            </Key>
          ) : null}
          {i.state === "failed" ? (
            <Key size="sm" icon="refresh" onClick={() => run("retry", () => api.queueAction("retry", i.id))}>
              Retry
            </Key>
          ) : null}
          {!["resolving", "downloading", "installing"].includes(i.state) ? (
            <Key size="sm" variant="ghost" icon="close" aria-label="Remove from queue" onClick={() => run("remove", () => api.queueAction("remove", i.id))} />
          ) : null}
        </span>
      </div>
    </div>
  );
}
