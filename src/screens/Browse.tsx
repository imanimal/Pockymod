// Browse: search on top, a filter rail of chips with counts, results as rows, and a detail module with the
// screen's one orange key, Install.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Deck } from "../components/Deck";
import { Description } from "../components/Description";
import { Icon } from "../components/Icon";
import { Chip, Empty, Grp, Key, Lcd, Module, Pill, SearchField, Seg, SwitchRow, Tag, SizeChip, Thumb, cx } from "../components/pocket";
import { api, pickFile } from "../lib/api";
import { ago, bytes, bytesParts, count, date, errMsg } from "../lib/format";
import { useAction, useApp } from "../lib/store";
import {
  ALL_FRAMEWORKS,
  FRAMEWORK_NAMES,
  FRAMEWORK_SHORT,
  KIND_COLOR,
  KIND_NAMES,
  emptyQuery,
  type BrowseQuery,
  type Category,
  type Compat,
  type Framework,
  type ModCard,
  type ModDetail,
  type ModKind,
  type ModPage,
  type ModioMod,
  type SortKey,
} from "../lib/types";

const SORTS: { value: SortKey; label: string }[] = [
  { value: "relevance", label: "Relevance" },
  { value: "downloads", label: "Downloads" },
  { value: "endorsements", label: "Endorsed" },
  { value: "updated", label: "Updated" },
  { value: "created", label: "New" },
  { value: "size", label: "Size" },
];

type Since = "any" | "7" | "30" | "365";

export function compatPill(c: Compat) {
  switch (c) {
    case "updated":
      return <Pill tone="ok">Updated since your patch</Pill>;
    case "likely":
      return <Pill>Archive only, likely fine</Pill>;
    case "risky":
      return <Pill tone="soon">Older than your patch</Pill>;
    default:
      return null;
  }
}

export function FrameworkTags({ list }: { list: Framework[] }) {
  return (
    <>
      {list.map((f) => (
        <Tag key={f} color="var(--k4)" title={`Needs ${FRAMEWORK_NAMES[f]}`}>
          {FRAMEWORK_SHORT[f]}
        </Tag>
      ))}
    </>
  );
}

export function KindTag({ kind }: { kind: ModKind | null | undefined }) {
  if (!kind) return null;
  return <Tag color={KIND_COLOR[kind]}>{KIND_NAMES[kind]}</Tag>;
}

export function Browse() {
  const [source, setSource] = useState<"nexus" | "modio">("nexus");
  const [modio, setModio] = useState(false);
  useEffect(() => {
    api.settings().then((s) => setModio(s.experiments.modio)).catch(() => {});
  }, []);
  if (modio && source === "modio") return <ModioBrowse onBack={() => setSource("nexus")} />;
  return <NexusBrowse onModio={modio ? () => setSource("modio") : undefined} />;
}

function ModioBrowse({ onBack }: { onBack: () => void }) {
  const [text, setText] = useState("");
  const [list, setList] = useState<ModioMod[]>([]);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const t = setTimeout(() => {
      api
        .modioBrowse(text, 0)
        .then((r) => {
          setList(r.mods);
          setTotal(r.total);
          setError(null);
        })
        .catch((e) => setError(errMsg(e)));
    }, 400);
    return () => clearTimeout(t);
  }, [text]);
  return (
    <div className="modio">
      <Deck>
        <Key icon="browse" onClick={onBack}>
          Back to Nexus
        </Key>
      </Deck>
      <Module title="mod.io (read-only experiment)" icon="browse" count={count(total)} bodyClassName="list-body">
        <div className="fbar">
          <SearchField value={text} onChange={setText} placeholder="Search mod.io" />
        </div>
        <p className="note pad">Read-only: subscribe and install these from the game's own mod menu. Whether third-party apps may use Cyberpunk's mod.io catalogue is unconfirmed.</p>
        {error ? (
          <Empty icon="warn" title="mod.io isn't available">{error}</Empty>
        ) : (
          <div className="rows">
            {list.map((m) => (
              <div key={m.id} className="row mrow brow">
                <Thumb src={m.logoUrl} alt="" />
                <div className="m-main">
                  <span className="m-t">{m.name}</span>
                  <span className="m-meta">
                    <span>{m.author}</span>
                    <span>{count(m.downloads)} downloads</span>
                  </span>
                </div>
                <span />
                <div className="m-due">
                  <SizeChip bytes={m.sizeBytes} />
                  <span className="m-abs">{ago(m.updatedAt)}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </Module>
    </div>
  );
}

function NexusBrowse({ onModio }: { onModio?: () => void }) {
  const { status, go, toast, fail, libraryRev } = useApp();
  const [q, setQ] = useState<BrowseQuery>(emptyQuery());
  const [text, setText] = useState("");
  const [since, setSince] = useState<Since>("any");
  const [pages, setPages] = useState<ModPage[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cats, setCats] = useState<Category[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const reqId = useRef(0);

  // the search text applies after a short pause in typing
  useEffect(() => {
    const t = setTimeout(() => setQ((old) => (old.text === text ? old : { ...old, text, offset: 0 })), 400);
    return () => clearTimeout(t);
  }, [text]);

  useEffect(() => {
    const after = since === "any" ? null : Math.floor(Date.now() / 1000) - Number(since) * 86400;
    setQ((old) => ({ ...old, updatedAfter: after, offset: 0 }));
  }, [since]);

  const load = useCallback(
    (query: BrowseQuery, append: boolean) => {
      if (!status?.hasKey) return;
      const id = ++reqId.current;
      setLoading(true);
      setError(null);
      api
        .browse(query)
        .then((p) => {
          if (id !== reqId.current) return;
          setPages((old) => (append ? [...old, p] : [p]));
          if (!append && p.mods.length && selected == null) setSelected(p.mods[0].modId);
        })
        .catch((e) => id === reqId.current && setError(errMsg(e)))
        .finally(() => id === reqId.current && setLoading(false));
    },
    // selected only matters for the first pick
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [status?.hasKey],
  );

  useEffect(() => load(q, false), [q, load, libraryRev]);
  useEffect(() => {
    if (status?.hasKey) api.categories().then(setCats).catch(() => {});
  }, [status?.hasKey]);

  const mods = useMemo(() => pages.flatMap((p) => p.mods), [pages]);
  const first = pages[0];
  const facets = first?.facets;
  const total = first?.total ?? 0;
  const loaded = pages.reduce((s, p) => s + p.mods.length + p.filteredOut, 0);

  const toggle = <K extends "categories" | "frameworks" | "kinds" | "tags">(key: K, v: BrowseQuery[K][number]) =>
    setQ((old) => {
      const list = old[key] as string[];
      const next = list.includes(v as string) ? list.filter((x) => x !== v) : [...list, v as string];
      return { ...old, [key]: next, offset: 0 };
    });

  const activeChips = [
    ...q.categories.map((c) => ({ label: c, onRemove: () => toggle("categories", c) })),
    ...q.tags.map((c) => ({ label: `#${c}`, onRemove: () => toggle("tags", c) })),
    ...q.frameworks.map((f) => ({ label: `needs ${FRAMEWORK_SHORT[f]}`, onRemove: () => toggle("frameworks", f) })),
    ...q.kinds.map((k) => ({ label: KIND_NAMES[k], onRemove: () => toggle("kinds", k) })),
    ...(q.compatibleOnly ? [{ label: "works with my game", onRemove: () => setQ({ ...q, compatibleOnly: false }) }] : []),
  ];
  const filterCount = activeChips.length + (q.downloaded != null ? 1 : 0) + (since !== "any" ? 1 : 0) + (q.minDownloads ? 1 : 0) + (q.maxSizeKb ? 1 : 0);

  const catCounts = new Map((facets?.categories ?? []).map((c) => [c.value, c.count]));
  const catList = facets?.categories.length ? facets.categories.map((c) => c.value) : cats.filter((c) => !c.parent).map((c) => c.name);
  const fwCount = (f: Framework) => facets?.frameworks.find((x) => x.value === f)?.count ?? 0;
  const kindCount = (k: ModKind) => facets?.kinds.find((x) => x.value === k)?.count ?? 0;

  const installFile = async () => {
    const p = await pickFile("Install a mod archive", ["zip", "7z", "rar"]);
    if (!p) return;
    try {
      const r = await api.installFile(p);
      toast(`Installed ${r.name}: ${r.placed} file(s) placed${r.unplaced.length ? `, ${r.unplaced.length} left out` : ""}.`);
    } catch (e) {
      fail(e);
    }
  };

  if (!status) return null;
  if (!status.hasKey) {
    return (
      <div className="browse-empty">
        <Module title="Browse" icon="browse" auto>
          <Empty icon="key" title="Connect your Nexus account" actions={<Key variant="primary" icon="key" onClick={() => go("settings")}>Add your API key</Key>}>
            Browsing uses your own personal Nexus API key, kept in Windows Credential Manager.
          </Empty>
        </Module>
      </div>
    );
  }

  return (
    <div className="browse">
      <Deck>
        <Key icon="file" onClick={installFile}>
          Install from file
        </Key>
        <Key icon="refresh" onClick={() => load({ ...q, offset: 0 }, false)}>
          Refresh
        </Key>
        {onModio ? (
          <Key icon="bolt" onClick={onModio}>
            mod.io (experiment)
          </Key>
        ) : null}
        <Key
          icon="close"
          disabled={filterCount === 0}
          onClick={() => {
            setQ({ ...emptyQuery(), text: q.text, sort: q.sort });
            setSince("any");
          }}
        >
          Clear filters
        </Key>
      </Deck>

      <Module title="Filters" icon="filter" count={filterCount || null} className="b-filters">
        <div className="frail">
          <div className="sect">Sort</div>
          <Seg label="Sort by" value={q.sort} onChange={(v) => setQ({ ...q, sort: v, offset: 0 })} options={SORTS} />
          <label className="swrow small">
            <span>Oldest or smallest first</span>
            <input type="checkbox" checked={q.ascending} onChange={(e) => setQ({ ...q, ascending: e.target.checked, offset: 0 })} />
          </label>

          <div className="sect">Requires</div>
          <div className="chips">
            {ALL_FRAMEWORKS.filter((f) => f !== "cybercmd" || fwCount(f) > 0).map((f) => (
              <Chip key={f} on={q.frameworks.includes(f)} count={fwCount(f)} onClick={() => toggle("frameworks", f)} title={FRAMEWORK_NAMES[f]}>
                {FRAMEWORK_SHORT[f]}
              </Chip>
            ))}
          </div>

          <div className="sect">Type</div>
          <div className="chips">
            {(["archive", "script", "redmod", "framework"] as ModKind[]).map((k) => (
              <Chip key={k} on={q.kinds.includes(k)} count={kindCount(k)} onClick={() => toggle("kinds", k)}>
                {KIND_NAMES[k]}
              </Chip>
            ))}
          </div>

          <div className="sect">Your game</div>
          <SwitchRow checked={q.compatibleOnly} onChange={(v) => setQ({ ...q, compatibleOnly: v })} hint={`${facets?.compatible ?? 0} on this page`}>
            Works with my game version
          </SwitchRow>
          <Seg
            label="Downloaded"
            value={q.downloaded == null ? "all" : q.downloaded ? "yes" : "no"}
            onChange={(v) => setQ({ ...q, downloaded: v === "all" ? null : v === "yes" })}
            options={[
              { value: "all", label: "All" },
              { value: "yes", label: "Downloaded", count: facets?.downloaded },
              { value: "no", label: "Not yet", count: facets?.notDownloaded },
            ]}
          />
          <SwitchRow checked={q.showHidden} onChange={(v) => setQ({ ...q, showHidden: v })} hint={facets?.hidden ? `${facets.hidden} hidden here` : undefined}>
            Show hidden mods
          </SwitchRow>

          <div className="sect">Updated</div>
          <Seg
            label="Updated within"
            value={since}
            onChange={setSince}
            options={[
              { value: "any", label: "Any time" },
              { value: "7", label: "Week" },
              { value: "30", label: "Month" },
              { value: "365", label: "Year" },
            ]}
          />

          <div className="sect">Numbers</div>
          <div className="fgrid ranges">
            <label htmlFor="mind">Downloads at least</label>
            <input id="mind" type="number" min={0} step={1000} value={q.minDownloads ?? ""} onChange={(e) => setQ({ ...q, minDownloads: e.target.value ? Number(e.target.value) : null, offset: 0 })} />
            <label htmlFor="minen">Endorsements at least</label>
            <input id="minen" type="number" min={0} step={100} value={q.minEndorsements ?? ""} onChange={(e) => setQ({ ...q, minEndorsements: e.target.value ? Number(e.target.value) : null, offset: 0 })} />
            <label htmlFor="maxs">Size at most (MB)</label>
            <input id="maxs" type="number" min={0} value={q.maxSizeKb ? Math.round(q.maxSizeKb / 1024) : ""} onChange={(e) => setQ({ ...q, maxSizeKb: e.target.value ? Number(e.target.value) * 1024 : null, offset: 0 })} />
          </div>

          <div className="sect">Category</div>
          <div className="chips">
            {catList.map((c) => (
              <Chip key={c} on={q.categories.includes(c)} count={catCounts.get(c) ?? null} onClick={() => toggle("categories", c)}>
                {c}
              </Chip>
            ))}
          </div>
          {facets?.tags.length ? (
            <>
              <div className="sect">Tags</div>
              <div className="chips">
                {facets.tags.slice(0, 16).map((t) => (
                  <Chip key={t.value} on={q.tags.includes(t.value)} count={t.count} onClick={() => toggle("tags", t.value)}>
                    {t.value}
                  </Chip>
                ))}
              </div>
            </>
          ) : null}
          <p className="note">Requires, type, version and downloaded counts are worked out on this computer, for the results loaded so far.</p>
        </div>
      </Module>

      <Module title="Results" icon="browse" count={first ? `${count(total)} on Nexus` : null} className="b-results" bodyClassName="list-body">
        <div className="fbar">
          <SearchField value={text} onChange={setText} placeholder="Search mods by name or description" chips={activeChips} autoFocus />
        </div>
        {error ? (
          <Empty icon="warn" title="Couldn't load mods" actions={<Key icon="refresh" onClick={() => load(q, false)}>Try again</Key>}>
            {error}
          </Empty>
        ) : mods.length === 0 && !loading ? (
          <Empty icon="browse" title="No mods match">
            {first?.filteredOut ? `${first.filteredOut} result(s) on this page were left out by local filters.` : "Try fewer filters or other words."}
          </Empty>
        ) : (
          <div className="rows anim" role="listbox" aria-label="Mods">
            <Grp count={`${mods.length} shown · ${loaded} of ${count(total)} loaded`}>{q.text ? `Results for “${q.text}”` : SORTS.find((s) => s.value === q.sort)?.label}</Grp>
            {mods.map((m, i) => (
              <ModRow key={m.modId} m={m} i={i} selected={selected === m.modId} onSelect={() => setSelected(m.modId)} />
            ))}
            {loaded < total ? (
              <div className="more">
                <Key icon="down" disabled={loading} onClick={() => {
                  const next = { ...q, offset: q.offset + loaded };
                  load(next, true);
                }}>
                  {loading ? "Loading…" : "Load more"}
                </Key>
              </div>
            ) : null}
          </div>
        )}
      </Module>

      <DetailPanel modId={selected} onHidden={() => load({ ...q, offset: 0 }, false)} />
    </div>
  );
}

function ModRow({ m, i, selected, onSelect }: { m: ModCard; i: number; selected: boolean; onSelect: () => void }) {
  return (
    <div
      className={cx("row mrow brow", selected && "sel")}
      role="option"
      aria-selected={selected}
      tabIndex={0}
      style={{ ["--i" as string]: i }}
      onClick={onSelect}
      onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onSelect()}
    >
      <Thumb src={m.thumbnailUrl} alt="" />
      <div className="m-main">
        <span className="m-t">
          {m.name}
          {m.hidden ? <span className="dim"> · hidden</span> : null}
        </span>
        <span className="m-meta">
          <span>{m.author}</span>
          {m.category ? <span>{m.category}</span> : null}
          <span>{count(m.downloads)} downloads</span>
        </span>
        <span className="m-tags">
          <KindTag kind={m.derived.kind} />
          <FrameworkTags list={m.derived.frameworks} />
          {m.installed ? <Pill tone="ok">Installed</Pill> : m.downloaded ? <Pill>Downloaded</Pill> : null}
        </span>
      </div>
      <span />
      <div className="m-due">
        <SizeChip bytes={m.sizeBytes} />
        <span className="m-abs">{ago(m.updatedAt)}</span>
      </div>
    </div>
  );
}

function DetailPanel({ modId, onHidden }: { modId: number | null; onHidden: () => void }) {
  const { status, go, toast, fail } = useApp();
  const [d, setD] = useState<ModDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [fileId, setFileId] = useState<number | null>(null);
  const [more, setMore] = useState(false);
  const { busy, run } = useAction();

  useEffect(() => {
    if (modId == null) return;
    let live = true;
    setD(null);
    setError(null);
    setMore(false);
    api
      .modDetail(modId)
      .then((x) => {
        if (!live) return;
        setD(x);
        setFileId(x.recommendedFileId);
      })
      .catch((e) => live && setError(errMsg(e)));
    return () => {
      live = false;
    };
  }, [modId]);

  if (modId == null) return <Module title="Mod" icon="chip" className="b-detail"><Empty icon="chip" title="Pick a mod">Its details and the Install key show here.</Empty></Module>;
  if (error) return <Module title="Mod" icon="chip" className="b-detail"><Empty icon="warn" title="Couldn't load this mod">{error}</Empty></Module>;
  if (!d) return <Module title="Mod" icon="chip" className="b-detail"><div className="empty">Loading…</div></Module>;

  const c = d.card;
  const file = d.files.find((f) => f.fileId === fileId);
  const premium = status?.account?.isPremium ?? false;
  const installable = d.files.filter((f) => !["OLD_VERSION", "ARCHIVED"].includes(f.category.toUpperCase()));
  const [sizeNum, sizeUnit] = bytesParts(file?.sizeBytes ?? c.sizeBytes);
  const needs = c.derived.frameworks;

  const install = () =>
    run("install", async () => {
      const ids = await api.queueMod(c.modId, fileId);
      const deps = ids.length - 1;
      if (premium && !d.manualOnly) toast(`Downloading ${c.name}${deps ? ` and ${deps} framework(s) it needs` : ""}.`);
      else toast(`${c.name}${deps ? ` and ${deps} framework(s)` : ""} queued: ${ids.length} click(s) on Nexus.`, { action: { label: "Open queue", run: () => go("downloads") } });
    });

  return (
    <Module title="Mod" icon="chip" count={c.version ? `v${c.version}` : null} className="b-detail" bodyClassName="detail-body">
      <h2 className="d-title">{c.name}</h2>
      <div className="meta-row">
        <span>by {c.author}</span>
        {c.category ? <span className="dim">· {c.category}</span> : null}
        <span className="dim">· updated {date(c.updatedAt)}</span>
      </div>
      {c.pictureUrl ? <img className="hero-img" src={c.pictureUrl} alt="" /> : null}
      <p className="summary">{c.summary}</p>
      <div className="lcds">
        <Lcd label="Downloads" value={count(c.downloads)} size="sm" />
        <Lcd label="Endorsed" value={count(c.endorsements)} size="sm" />
        <Lcd label="Size" value={sizeNum} unit={sizeUnit} size="sm" />
      </div>
      <div className="chips">
        <KindTag kind={c.derived.kind} />
        <FrameworkTags list={needs} />
        {compatPill(c.compat)}
        {d.manualOnly ? <Pill tone="soon">Manual download</Pill> : null}
      </div>

      {installable.length > 1 ? (
        <div className="fgrid">
          <label htmlFor="file">File</label>
          <select id="file" value={fileId ?? ""} onChange={(e) => setFileId(Number(e.target.value))}>
            {installable.map((f) => (
              <option key={f.fileId} value={f.fileId}>
                {f.name} {f.version ? `(${f.version})` : ""} · {f.category.toLowerCase().replace("_", " ")} · {bytes(f.sizeBytes)}
              </option>
            ))}
          </select>
        </div>
      ) : null}

      <div className="actions">
        <Key variant="primary" size="lg" icon="download" disabled={!fileId || busy === "install"} onClick={install}>
          {c.installed ? "Reinstall" : "Install"}
        </Key>
        <Key icon="eye" onClick={() => run("hide", async () => { await api.setHidden(c.modId, !c.hidden); setD({ ...d, card: { ...c, hidden: !c.hidden } }); onHidden(); })}>
          {c.hidden ? "Unhide" : "Hide"}
        </Key>
        <Key
          icon="star"
          pressed={d.endorseStatus === "Endorsed"}
          disabled={busy === "endorse"}
          onClick={() =>
            run("endorse", async () => {
              const on = d.endorseStatus !== "Endorsed";
              await api.endorse(c.modId, on);
              setD({ ...d, endorseStatus: on ? "Endorsed" : "Abstained" });
            })
          }
        >
          {d.endorseStatus === "Endorsed" ? "Endorsed" : "Endorse"}
        </Key>
        <Key
          icon="bell"
          pressed={!!d.tracked}
          onClick={() =>
            run("track", async () => {
              await api.setTracked(c.modId, !d.tracked);
              setD({ ...d, tracked: !d.tracked });
            })
          }
        >
          {d.tracked ? "Tracked" : "Track"}
        </Key>
        <Key variant="ghost" icon="link" onClick={() => api.openNexus(c.modId).catch(fail)}>
          Nexus page
        </Key>
      </div>
      <p className="note">
        {premium && !d.manualOnly
          ? `Premium: one click downloads and installs it${needs.length ? ", with the frameworks it needs" : ""}.`
          : `Free account: the queue opens the Nexus page here and you click “Mod Manager Download”.${needs.length ? " Missing frameworks are queued too." : ""}`}
      </p>

      {d.requirements.length ? (
        <>
          <div className="sect">Requirements</div>
          <ul className="reqs">
            {d.requirements.map((r) => (
              <li key={`${r.modId}-${r.name}`}>
                <Icon name={r.external ? "link" : "chip"} />
                <span>{r.name}</span>
                {r.notes ? <span className="dim"> · {r.notes}</span> : null}
              </li>
            ))}
          </ul>
        </>
      ) : null}

      <div className="sect">Description</div>
      <div className={cx("desc-wrap", more && "open")}>
        <Description source={d.description} onLink={(href) => {
          const m = href.match(/nexusmods\.com\/cyberpunk2077\/mods\/(\d+)/);
          if (m) api.openNexus(Number(m[1])).catch(fail);
          else toast(`Links outside Nexus aren't opened from here: ${href}`);
        }} />
      </div>
      <Key variant="ghost" size="sm" icon={more ? "up" : "down"} onClick={() => setMore(!more)}>
        {more ? "Less" : "More"}
      </Key>
      {d.previewFiles.length ? (
        <>
          <div className="sect">In the archive</div>
          <ul className="files">
            {d.previewFiles.slice(0, 12).map((f) => (
              <li key={f} className="mono">
                {f}
              </li>
            ))}
            {d.previewFiles.length > 12 ? <li className="dim">and {d.previewFiles.length - 12} more</li> : null}
          </ul>
        </>
      ) : null}
    </Module>
  );
}
