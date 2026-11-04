// Downloads: the queue. LCDs for mods left, clicks needed and time left; amber while the queue waits on the user.
// When it waits for a click, the orange band and key point at the one thing to do next.
import { plural } from "../lib/text";
import { Icon } from "../ui/Icon";
import { Empty, Key, Lcd, Module, Pill, bytesText, type PillTone } from "../ui/pocket";
import { errorText, queue, type ItemState, type QueueItem, type QueueSnapshot } from "../lib/api";
import { toast } from "../ui/toast";

function stateLabel(s: ItemState): { text: string; tone?: PillTone } {
  switch (s.state) {
    case "waiting": return { text: "Waiting" };
    case "needsClick": return { text: "Needs your click", tone: "hot" };
    case "downloading": return { text: s.total ? `${Math.round((s.received / s.total) * 100)}%` : bytesText(s.received) };
    case "installing": return { text: "Installing" };
    case "done": return { text: s.note ? "Done, see note" : "Installed", tone: s.note ? "soon" : "ok" };
    case "failed": return { text: "Failed", tone: "over" };
  }
}

const clock = (secs: number | null) => {
  if (secs === null) return "--:--";
  const m = Math.floor(secs / 60), s = secs % 60;
  return m >= 100 ? `${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}` : `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
};

export function Downloads({ snap }: { snap: QueueSnapshot | null }) {
  const s = snap?.summary;
  const items = snap?.items ?? [];
  const current = items.find((i) => i.state.state === "downloading");
  const waiting = items.find((i) => i.state.state === "needsClick");
  const act = (p: Promise<unknown>) => p.catch((e) => toast(errorText(e), true));

  return (
    <div className="stack">
      <div className="lcds">
        <Lcd label="Mods left" value={String(s?.left ?? 0).padStart(2, "0")} sub={s?.running ? "Working" : s?.left ? "Queued" : "Queue is empty"} />
        <Lcd label="Needs your click" value={String(s?.needsClick ?? 0).padStart(2, "0")} tone={s?.waitingOnUser ? "hot" : "ok"}
          sub={s?.premium ? "Premium: no clicks" : s?.waitingOnUser ? "Waiting for you on Nexus" : "One per file on a free account"} />
        <Lcd label="Time left" value={clock(s?.secondsLeft ?? null)} sub={s?.bytesPerSec ? `${bytesText(s.bytesPerSec)}/s · ${bytesText(s.bytesLeft)} to go` : s?.bytesLeft ? `${bytesText(s.bytesLeft)} to go` : "Nothing to download"}
          bar={current && current.state.state === "downloading" && current.state.total ? current.state.received / current.state.total : undefined} />
      </div>

      {waiting && (
        <Module icon="downloads" title="Download next" count={waiting.modName} band="hot">
          <p>Press <b className="hi">Mod Manager Download</b> on the Nexus page for <b className="hi">{waiting.fileName}</b>. The queue goes on by itself when the link arrives.</p>
          <div className="actions" style={{ marginTop: ".7em" }}>
            <Key variant="dark" icon="web" onClick={() => act(queue.showPage(waiting.id))}>Show the Nexus page</Key>
            <Key variant="ghost" onClick={() => act(queue.remove(waiting.id))}>Skip this mod</Key>
          </div>
          {waiting.manualOnly && <div className="note warn" style={{ marginTop: ".7em" }}>This file is not a .zip, .7z or .rar, so it cannot be installed for you. Follow the steps on its page.</div>}
        </Module>
      )}

      <Module icon="downloads" title="Queue" count={items.length ? plural(items.length, "item") : undefined} bodyClass="list"
        tools={items.some((i) => ["done", "failed"].includes(i.state.state)) ? <Key size="sm" variant="ghost" onClick={() => act(queue.clearFinished())}>Clear finished</Key> : undefined}>
        {items.length === 0 && <Empty icon="downloads" title="Nothing queued">Press Install on a mod in Browse. Its requirements come along.</Empty>}
        <div className="rows">
          {items.map((i) => <QueueRow key={i.id} i={i} onRetry={() => act(queue.retry(i.id))} onRemove={() => act(queue.remove(i.id))} />)}
        </div>
      </Module>
    </div>
  );
}

function QueueRow({ i, onRetry, onRemove }: { i: QueueItem; onRetry: () => void; onRemove: () => void }) {
  const l = stateLabel(i.state);
  const busy = i.state.state === "downloading" || i.state.state === "installing";
  return (
    <div className={`row${i.state.state === "needsClick" ? " sel" : ""}`}>
      <span className="m-main">
        <span className="m-t">{i.modName}</span>
        <span className="m-meta"><span>{i.fileName}</span>{i.version && <span>v{i.version}</span>}{i.requiredBy && <span>needed by {i.requiredBy}</span>}</span>
        {i.state.state === "failed" && <span className="badc" style={{ fontSize: ".84em" }}>{i.state.message}</span>}
        {i.state.state === "done" && i.state.note && <span className="warnc" style={{ fontSize: ".84em" }}>{i.state.note}</span>}
      </span>
      <span className="m-side" style={{ flexDirection: "row", alignItems: "center", gap: ".5em" }}>
        <span>{i.size ? bytesText(i.size) : ""}</span>
        <Pill tone={l.tone}>{l.text}</Pill>
        {i.state.state === "failed" && <Key size="sm" icon="refresh" onClick={onRetry} aria-label="Retry" title="Retry" />}
        {!busy && i.state.state !== "done" && <Key size="sm" variant="ghost" icon="x" onClick={onRemove} aria-label="Remove" title="Remove" />}
        {i.state.state === "downloading" && <Key size="sm" variant="ghost" icon="x" onClick={() => void queue.cancel(i.id).catch(() => {})} aria-label="Stop this download" title="Stop (it resumes from here on Retry)" />}
        {i.state.state === "installing" && <Icon name="clock" />}
      </span>
    </div>
  );
}
