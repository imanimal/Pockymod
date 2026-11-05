// Collections: browse Nexus collections with pre-install checks (game version, missing mods, how many clicks),
// and make simple local ones to export as a shareable file. Publishing to Nexus is an experiment behind a flag.

import { useEffect, useState } from "react";
import { Deck } from "../components/Deck";
import { Empty, Grp, Key, Lcd, Module, Pill, SearchField, Seg, SwitchRow, SizeChip, Thumb, cx } from "../components/pocket";
import { api, pickFile, pickSave } from "../lib/api";
import { bytes, bytesParts, count, errMsg } from "../lib/format";
import { useAction, useApp } from "../lib/store";
import type { Collection, CollectionCard, LibraryRow, PreCheck, Settings } from "../lib/types";

type Tab = "nexus" | "mine";

export function Collections() {
  const { focus } = useApp();
  const f = focus as { exportKeys?: string[]; slug?: string; revision?: number | null } | null;
  const [tab, setTab] = useState<Tab>(f?.exportKeys ? "mine" : "nexus");
  return (
    <div className="collections">
      <div className="tabs">
        <Seg
          label="Collections"
          value={tab}
          onChange={setTab}
          options={[
            { value: "nexus", label: "On Nexus" },
            { value: "mine", label: "Mine" },
          ]}
        />
      </div>
      {tab === "nexus" ? <NexusCollections initialSlug={f?.slug} initialRevision={f?.revision ?? null} /> : <MyCollections initialKeys={f?.exportKeys} />}
    </div>
  );
}

function CheckView({ check, collection, slug, primary = true }: { check: PreCheck; collection: Collection; slug: string; primary?: boolean }) {
  const { go, toast } = useApp();
  const { busy, run } = useAction();
  const [optional, setOptional] = useState(false);
  const [c, setC] = useState(check);
  const [sizeNum, sizeUnit] = bytesParts(c.bytesToDownload);
  useEffect(() => setC(check), [check]);
  useEffect(() => {
    api.collectionPrecheck(collection, optional).then(setC).catch(() => {});
  }, [optional, collection]);
  const versionTone = c.gameVersionOk === false ? "bad" : null;
  return (
    <div className="detail-body">
      <h2 className="d-title">{collection.info.name}</h2>
      <div className="meta-row">
        <span>by {collection.info.author || "unknown"}</span>
        {c.gameVersions.length ? <span className="dim">· for game {c.gameVersions.join(", ")}</span> : null}
      </div>
      <p className="summary">{collection.info.description}</p>
      <div className="lcds">
        <Lcd label="Mods" value={c.total} sub={`${c.installed} already installed`} size="sm" />
        <Lcd label="Missing" value={c.missing.length} sub={`${sizeNum} ${sizeUnit} to download`} size="sm" />
        <Lcd label="Clicks" value={c.manualClicks} tone={c.manualClicks ? "hot" : null} sub={c.manualClicks ? "on Nexus, by you" : "fully automatic"} size="sm" />
        <Lcd label="Game" value={c.gameVersionOk == null ? "?" : c.gameVersionOk ? "OK" : "NO"} tone={versionTone} sub={c.gameVersionOk === false ? "made for another version" : c.gameVersionOk ? "matches yours" : "no version given"} size="sm" />
      </div>
      {c.optional ? (
        <SwitchRow checked={optional} onChange={setOptional} hint={`${c.optional} optional mod(s)`}>
          Include optional mods
        </SwitchRow>
      ) : null}
      <div className="actions">
        <Key
          variant={primary ? "primary" : "dark"}
          size="lg"
          icon="download"
          disabled={c.missing.filter((m) => !m.external).length === 0 || busy === "install"}
          onClick={() =>
            run("install", async () => {
              const ids = await api.collectionInstall(collection, slug, optional);
              toast(`${ids.length} mod(s) queued from ${collection.info.name}.`, { action: { label: "Open queue", run: () => go("downloads") } });
            })
          }
        >
          {c.missing.length === 0 ? "All installed" : `Install ${c.missing.filter((m) => !m.external).length} mods`}
        </Key>
      </div>
      {c.gameVersionOk === false ? <p className="note badc">This collection was made for game version {c.gameVersions.join(", ")}. Mods in it may not work with yours.</p> : null}
      {c.missing.length ? (
        <>
          <div className="sect">Missing</div>
          <div className="rows">
            {c.missing.map((m) => (
              <div key={`${m.modId}-${m.name}`} className="row mrow crow">
                <span />
                <div className="m-main">
                  <span className="m-t">{m.name}</span>
                  <span className="m-meta">
                    <span>{m.external ? "Not on Nexus" : `Mod ${m.modId}`}</span>
                    <span>{bytes(m.sizeBytes)}</span>
                    {m.optional ? <span>optional</span> : null}
                  </span>
                </div>
                <span />
                <div className="m-due">{m.external ? <Pill tone="soon">By hand</Pill> : <Pill>Queue</Pill>}</div>
              </div>
            ))}
          </div>
        </>
      ) : null}
    </div>
  );
}

function NexusCollections({ initialSlug, initialRevision }: { initialSlug?: string; initialRevision: number | null }) {
  const { status, go } = useApp();
  const [text, setText] = useState("");
  const [sort, setSort] = useState<"endorsements" | "downloads" | "updated">("endorsements");
  const [matching, setMatching] = useState(true);
  const [list, setList] = useState<CollectionCard[]>([]);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [slug, setSlug] = useState<string | null>(initialSlug ?? null);
  const [loaded, setLoaded] = useState<{ collection: Collection; check: PreCheck } | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);

  useEffect(() => {
    if (!status?.hasKey) return;
    const t = setTimeout(() => {
      api
        .collections(text, matching, sort, 0)
        .then((r) => {
          setList(r.collections);
          setTotal(r.total);
          setError(null);
        })
        .catch((e) => setError(errMsg(e)));
    }, 300);
    return () => clearTimeout(t);
  }, [text, sort, matching, status?.hasKey]);

  useEffect(() => {
    if (!slug) return;
    setLoaded(null);
    setLoadErr(null);
    const rev = slug === initialSlug ? initialRevision : list.find((c) => c.slug === slug)?.revision ?? null;
    api.collectionFetch(slug, rev, false).then(setLoaded).catch((e) => setLoadErr(errMsg(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [slug]);

  if (status && !status.hasKey) {
    return (
      <Module title="Collections" icon="collections" auto>
        <Empty icon="key" title="Connect your Nexus account" actions={<Key variant="primary" icon="key" onClick={() => go("settings")}>Add your API key</Key>} />
      </Module>
    );
  }

  return (
    <div className="split">
      <Deck>
        <Key icon="link" onClick={() => go("downloads")}>
          Open the queue
        </Key>
      </Deck>
      <Module title="Collections on Nexus" icon="collections" count={count(total)} bodyClassName="list-body">
        <div className="fbar">
          <SearchField value={text} onChange={setText} placeholder="Search collections" />
        </div>
        <div className="qstrip">
          <Seg
            label="Sort"
            value={sort}
            onChange={setSort}
            options={[
              { value: "endorsements", label: "Endorsed" },
              { value: "downloads", label: "Downloads" },
              { value: "updated", label: "Updated" },
            ]}
          />
          <SwitchRow checked={matching} onChange={setMatching}>
            My game version
          </SwitchRow>
        </div>
        {error ? (
          <Empty icon="warn" title="Couldn't load collections">{error}</Empty>
        ) : (
          <div className="rows">
            <Grp count={list.length}>Collections</Grp>
            {list.map((c) => (
              <div key={c.slug} className={cx("row mrow brow", slug === c.slug && "sel")} tabIndex={0} role="option" aria-selected={slug === c.slug} onClick={() => setSlug(c.slug)}>
                <Thumb src={c.thumbnailUrl} alt="" />
                <div className="m-main">
                  <span className="m-t">{c.name}</span>
                  <span className="m-meta">
                    <span>{c.author}</span>
                    <span>{c.modCount} mods</span>
                    <span>{count(c.downloads)} downloads</span>
                  </span>
                  <span className="m-tags">
                    {c.gameVersions.map((v) => (
                      <Pill key={v}>game {v}</Pill>
                    ))}
                  </span>
                </div>
                <span />
                <div className="m-due">
                  <SizeChip bytes={c.sizeBytes} />
                  <span className="m-abs">rev {c.revision ?? "–"}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </Module>
      <Module title="Pre-install check" icon="check" className="c-detail">
        {!slug ? (
          <Empty icon="collections" title="Pick a collection">Pocket Mods reads its manifest and checks it against your game before anything downloads.</Empty>
        ) : loadErr ? (
          <Empty icon="warn" title="Couldn't read this collection">{loadErr}</Empty>
        ) : !loaded ? (
          <div className="empty">Reading the manifest…</div>
        ) : (
          <CheckView check={loaded.check} collection={loaded.collection} slug={slug} />
        )}
      </Module>
    </div>
  );
}

function MyCollections({ initialKeys }: { initialKeys?: string[] }) {
  const { toast, fail, status } = useApp();
  const [rows, setRows] = useState<LibraryRow[]>([]);
  const [keys, setKeys] = useState<Set<string>>(new Set(initialKeys ?? []));
  const [name, setName] = useState("My collection");
  const [author, setAuthor] = useState(status?.account?.name ?? "");
  const [desc, setDesc] = useState("");
  const [saved, setSaved] = useState<{ name: string; collection: Collection }[]>([]);
  const [imported, setImported] = useState<{ collection: Collection; check: PreCheck } | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const { busy, run } = useAction();

  const reload = () => {
    api.library().then(setRows).catch(() => {});
    api.savedCollections().then(setSaved).catch(() => {});
    api.settings().then(setSettings).catch(() => {});
  };
  useEffect(reload, []);

  const exportNow = () =>
    run("export", async () => {
      const path = await pickSave("Save the collection", `${name.replace(/[^\w -]/g, "_")}.json`);
      if (!path) return;
      const c = await api.collectionExport(name, author, desc, [...keys], path);
      toast(`Saved ${c.info.name} with ${c.mods.length} mod(s), pinned to their files and hashes.`);
      reload();
    });

  const importFile = async () => {
    const p = await pickFile("Open a collection file", ["json"]);
    if (!p) return;
    try {
      setImported(await api.collectionImport(p, false));
    } catch (e) {
      fail(e);
    }
  };

  const toggle = (k: string) => setKeys((s) => {
    const n = new Set(s);
    if (n.has(k)) n.delete(k);
    else n.add(k);
    return n;
  });

  return (
    <div className="split">
      <Deck>
        <Key icon="file" onClick={importFile}>
          Open a collection file
        </Key>
        {settings?.experiments.publishCollections ? (
          <Key icon="up" onClick={() => api.collectionPublish().catch(fail)}>
            Publish to Nexus (experiment)
          </Key>
        ) : null}
      </Deck>
      <Module title="New collection" icon="plus" count={`${keys.size} picked`} bodyClassName="list-body">
        <div className="fgrid pad">
          <label htmlFor="cn">Name</label>
          <input id="cn" value={name} onChange={(e) => setName(e.target.value)} />
          <label htmlFor="ca">Author</label>
          <input id="ca" value={author} onChange={(e) => setAuthor(e.target.value)} />
          <label htmlFor="cd">Description</label>
          <textarea id="cd" rows={2} value={desc} onChange={(e) => setDesc(e.target.value)} />
        </div>
        <div className="actions pad">
          <Key variant="primary" icon="file" disabled={keys.size === 0 || !name.trim() || busy === "export"} onClick={exportNow}>
            Export {keys.size} mod{keys.size === 1 ? "" : "s"}
          </Key>
          <Key variant="ghost" size="sm" onClick={() => setKeys(new Set(rows.map((r) => r.key)))}>
            Pick all
          </Key>
          <Key variant="ghost" size="sm" onClick={() => setKeys(new Set())}>
            Pick none
          </Key>
        </div>
        <p className="note pad">The file pins each mod's ID, file ID and MD5, the requirements between them, and your REDmod load order.</p>
        <div className="rows">
          <Grp count={rows.length}>Library</Grp>
          {rows.map((r) => (
            <label key={r.key} className={cx("row mrow pick", keys.has(r.key) && "sel")}>
              <input type="checkbox" checked={keys.has(r.key)} onChange={() => toggle(r.key)} />
              <div className="m-main">
                <span className="m-t">{r.name}</span>
                <span className="m-meta">
                  <span>{r.modId ? `Nexus ${r.modId}` : "Not from Nexus"}</span>
                  <span>{r.version ? `v${r.version}` : ""}</span>
                </span>
              </div>
            </label>
          ))}
        </div>
      </Module>
      <Module title={imported ? "Opened file" : "Saved"} icon={imported ? "check" : "collections"} count={imported ? null : saved.length} className="c-detail">
        {imported ? (
          <>
            <CheckView collection={imported.collection} check={imported.check} slug="file" primary={false} />
            <Key variant="ghost" size="sm" icon="close" onClick={() => setImported(null)}>
              Close
            </Key>
          </>
        ) : saved.length === 0 ? (
          <Empty icon="collections" title="Nothing saved yet">Collections you export, and Nexus ones you've opened, are kept here.</Empty>
        ) : (
          <div className="rows">
            {saved.map((s) => (
              <div key={s.name} className="row mrow crow">
                <span />
                <div className="m-main">
                  <span className="m-t">{s.collection.info.name}</span>
                  <span className="m-meta">
                    <span>{s.name.startsWith("nexus:") ? "From Nexus" : "Made here"}</span>
                    <span>{s.collection.mods.length} mods</span>
                  </span>
                </div>
                <span />
                <div className="m-due actions sm">
                  <Key size="sm" onClick={() => run("open", async () => setImported({ collection: s.collection, check: await api.collectionPrecheck(s.collection, false) }))}>
                    Check
                  </Key>
                  <Key size="sm" variant="ghost" icon="trash" aria-label="Delete" onClick={() => run("del", async () => { await api.deleteSavedCollection(s.name); reload(); })} />
                </div>
              </div>
            ))}
          </div>
        )}
      </Module>
    </div>
  );
}
