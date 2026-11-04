//! The download queue. Free accounts need one click on the Nexus site per file ("Mod Manager Download"), so the
//! queue shows that cost up front, opens each file's page in the app, catches the nxm:// link and moves on by itself.
//! Premium accounts get direct links, so the queue runs without stopping. The runner (in the app shell) does the I/O;
//! this module is the state machine.

use crate::browse::{self, main_file};
use crate::cyberpunk::frameworks;
use crate::db::Db;
use crate::error::Result;
use crate::nexus::nxm::NxmLink;
use crate::nexus::NexusClient;
use serde::Serialize;
use std::collections::{BTreeSet, VecDeque};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ItemState {
    Waiting,
    /// Free account: the file's page is open and waits for the user's click.
    NeedsClick,
    Downloading { received: u64, total: Option<u64> },
    Installing,
    /// Installed, or staged waiting for a decision (conflicts, copy fallback, unknown files).
    Done { install_id: Option<i64>, note: Option<String> },
    Failed { message: String },
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: u64,
    pub mod_id: u32,
    pub file_id: u32,
    pub mod_name: String,
    pub file_name: String,
    pub version: String,
    /// Nexus's file name (`uri`), used for the download.
    pub uri: String,
    pub size: u64,
    /// Added because another queued mod requires it.
    pub required_by: Option<String>,
    /// Not a .zip/.7z/.rar: it cannot be installed automatically; its page opens in the app instead.
    pub manual_only: bool,
    pub state: ItemState,
    #[serde(skip)]
    pub nxm: Option<NxmLink>,
    /// Part of a collection (which asks for this very file), not a requirement (any copy of the mod will do).
    #[serde(default)]
    pub from_collection: bool,
}

impl QueueItem {
    pub fn is_open(&self) -> bool { !matches!(self.state, ItemState::Done { .. } | ItemState::Failed { .. }) }
    pub fn is_busy(&self) -> bool { matches!(self.state, ItemState::Downloading { .. } | ItemState::Installing) }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub left: usize,
    /// How many still need a click on Nexus (free accounts).
    pub needs_click: usize,
    pub bytes_left: u64,
    pub running: bool,
    /// The queue is stopped on the user (a click, or a failure).
    pub waiting_on_user: bool,
    pub failed: usize,
    pub premium: bool,
    pub bytes_per_sec: Option<u64>,
    pub seconds_left: Option<u64>,
}

/// What the runner should do next.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Premium: ask Nexus for the link and download.
    FetchLink(u64),
    /// Free: a link has been caught; download with it.
    DownloadWith(u64),
    /// Free, or a manual-only file: show this item's page in the app and wait.
    OpenPage(u64),
}

#[derive(Debug, Default)]
pub struct Queue {
    pub items: Vec<QueueItem>,
    pub premium: bool,
    next_id: u64,
    pub bytes_per_sec: Option<u64>,
}

impl Queue {
    /// Add an item unless the same file is already queued and not failed. Returns its id.
    pub fn add(&mut self, mut item: QueueItem) -> u64 {
        if let Some(i) = self.items.iter().find(|i| i.mod_id == item.mod_id && i.file_id == item.file_id && !matches!(i.state, ItemState::Failed { .. })) {
            return i.id;
        }
        self.items.retain(|i| !(i.mod_id == item.mod_id && i.file_id == item.file_id));
        self.next_id += 1;
        item.id = self.next_id;
        item.state = ItemState::Waiting;
        let id = item.id;
        self.items.push(item);
        id
    }

    pub fn get(&self, id: u64) -> Option<&QueueItem> { self.items.iter().find(|i| i.id == id) }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut QueueItem> { self.items.iter_mut().find(|i| i.id == id) }
    pub fn set(&mut self, id: u64, state: ItemState) { if let Some(i) = self.get_mut(id) { i.state = state; } }
    pub fn remove(&mut self, id: u64) { self.items.retain(|i| i.id != id || i.is_busy()); }
    pub fn clear_finished(&mut self) { self.items.retain(|i| i.is_open()); }
    /// Try a failed item again. Its Nexus link is kept while it is still valid, so a retry needs no new click.
    pub fn retry(&mut self, id: u64) {
        if let Some(i) = self.get_mut(id) {
            if matches!(i.state, ItemState::Failed { .. }) {
                i.state = ItemState::Waiting;
                let now = crate::db::now();
                i.nxm = i.nxm.take().filter(|l| !l.expired(now));
            }
        }
    }

    pub fn summary(&self) -> Summary {
        let open: Vec<&QueueItem> = self.items.iter().filter(|i| i.is_open()).collect();
        let needs_click = if self.premium { open.iter().filter(|i| i.manual_only).count() } else { open.iter().filter(|i| i.nxm.is_none() && !i.is_busy()).count() };
        let bytes_left: u64 = open.iter().map(|i| match &i.state { ItemState::Downloading { received, .. } => i.size.saturating_sub(*received), _ => i.size }).sum();
        let running = open.iter().any(|i| i.is_busy());
        let failed = self.items.iter().filter(|i| matches!(i.state, ItemState::Failed { .. })).count();
        let waiting_on_user = !running && (open.iter().any(|i| i.state == ItemState::NeedsClick) || (failed > 0 && open.is_empty()));
        let seconds_left = self.bytes_per_sec.filter(|b| *b > 0).map(|b| bytes_left / b);
        Summary { left: open.len(), needs_click, bytes_left, running, waiting_on_user, failed, premium: self.premium, bytes_per_sec: self.bytes_per_sec, seconds_left }
    }

    /// The next step, one at a time and in order (dependencies were queued before the mods needing them).
    pub fn next_action(&self) -> Option<Action> {
        if self.items.iter().any(|i| i.is_busy()) { return None; }
        if let Some(i) = self.items.iter().find(|i| i.is_open() && i.nxm.is_some()) { return Some(Action::DownloadWith(i.id)); }
        let next = self.items.iter().find(|i| i.is_open() && matches!(i.state, ItemState::Waiting | ItemState::NeedsClick))?;
        if next.manual_only || !self.premium { Some(Action::OpenPage(next.id)) } else { Some(Action::FetchLink(next.id)) }
    }

    /// A caught nxm:// link: match it to the queued file, or queue it (the user clicked something else on Nexus).
    pub fn accept_nxm(&mut self, link: NxmLink, fallback: impl FnOnce(&NxmLink) -> QueueItem) -> u64 {
        if let Some(i) = self.items.iter_mut().find(|i| i.is_open() && i.mod_id == link.mod_id && i.file_id == link.file_id) {
            i.nxm = Some(link);
            i.state = ItemState::Waiting;
            return i.id;
        }
        // same mod, another file of it: the user picked a different file on the page, take that one
        if let Some(i) = self.items.iter_mut().find(|i| i.is_open() && !i.is_busy() && i.mod_id == link.mod_id) {
            i.file_id = link.file_id;
            i.nxm = Some(link);
            i.state = ItemState::Waiting;
            return i.id;
        }
        let mut item = fallback(&link);
        item.nxm = Some(link);
        let id = self.add(item.clone());
        if let Some(i) = self.get_mut(id) { i.nxm = item.nxm; }
        id
    }
}

/// Whether a Nexus file can be installed automatically. Its `uri` is usually the archive's name
/// (`Mod-123-1-0.zip`), but files listed in collections can carry a storage path with no extension
/// (`4c/ec/e3/4cece3d6-…`, seen live); those are taken as archives and checked by content when installed.
/// Keep the open items, so a restart (or a crash) does not lose the queue. Links from Nexus are not kept: they
/// expire, and the queue asks again.
pub fn save(db: &Db, q: &Queue) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    tx.execute("DELETE FROM queue_items", [])?;
    for i in q.items.iter().filter(|i| i.is_open() || matches!(i.state, ItemState::Failed { .. })) {
        tx.execute("INSERT INTO queue_items (id, data) VALUES (?1, ?2)", rusqlite::params![i.id as i64, serde_json::to_string(i).unwrap_or_default()])?;
    }
    tx.commit()?;
    Ok(())
}

/// The saved queue, with anything that was running set back to waiting.
pub fn load(db: &Db) -> Result<Queue> {
    let mut st = db.conn.prepare("SELECT data FROM queue_items ORDER BY id")?;
    let rows: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    let mut q = Queue::default();
    for r in rows {
        let Ok(mut i) = serde_json::from_str::<QueueItem>(&r) else { continue };
        if !matches!(i.state, ItemState::Failed { .. }) { i.state = ItemState::Waiting; }
        q.next_id = q.next_id.max(i.id);
        q.items.push(i);
    }
    Ok(q)
}

pub fn is_archive_name(uri: &str) -> bool {
    let name = uri.rsplit('/').next().unwrap_or(uri).to_lowercase();
    match name.rsplit_once('.') {
        Some((_, ext)) if ext.len() <= 4 && ext.chars().all(|c| c.is_ascii_alphanumeric()) && ext.chars().any(|c| c.is_ascii_alphabetic()) => matches!(ext, "zip" | "7z" | "rar"),
        _ => true,
    }
}

/// The local file name for a download: the archive's name, or `<mod>-<file>-<id>` for storage paths.
pub fn download_name(mod_id: u32, file_id: u32, uri: &str) -> String {
    let last = uri.rsplit('/').next().unwrap_or(uri);
    if uri.contains('/') || !last.contains('.') { format!("{mod_id}-{file_id}-{last}") } else { last.to_string() }
}

/// A mod's file and every Nexus mod it requires that is not installed, dependencies first.
/// Frameworks already detected in the game folder count as installed.
pub async fn with_dependencies(client: &NexusClient, db: &Mutex<Db>, mod_id: u32, file_id: u32, already: &BTreeSet<u32>) -> Result<Vec<QueueItem>> {
    let mut out: Vec<QueueItem> = Vec::new();
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    let mut todo: VecDeque<(u32, Option<u32>, Option<String>, u32)> = VecDeque::from([(mod_id, Some(file_id), None, 0)]);
    while let Some((id, fid, required_by, depth)) = todo.pop_front() {
        if !seen.insert(id) { continue; }
        let d = browse::detail(client, db, id, None).await?;
        let file = match fid { Some(f) => d.files.iter().find(|x| x.file_id == f), None => main_file(&d.files) };
        let Some(file) = file else { continue };
        out.push(QueueItem {
            id: 0, mod_id: id, file_id: file.file_id, mod_name: d.detail.name.clone(), file_name: file.name.clone(), version: file.version.clone(),
            uri: file.uri.clone(), size: file.bytes(), required_by: required_by.clone(), manual_only: !is_archive_name(&file.uri), state: ItemState::Waiting, nxm: None, from_collection: false,
        });
        if depth >= 4 { continue; }
        for r in &d.detail.mod_requirements.nexus_requirements.nodes {
            if r.external_requirement || already.contains(&r.mod_id) || seen.contains(&r.mod_id) { continue; }
            todo.push_back((r.mod_id, None, Some(d.detail.name.clone()), depth + 1));
        }
    }
    // dependencies first: deepest last-found first, the asked-for mod at the end; frameworks before everything
    out.reverse();
    out.sort_by_key(|i| frameworks::by_mod_id(i.mod_id).is_none());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(m: u32, f: u32, uri: &str) -> QueueItem {
        QueueItem { id: 0, mod_id: m, file_id: f, mod_name: format!("m{m}"), file_name: "f".into(), version: "1".into(), uri: uri.into(), size: 1000,
            required_by: None, manual_only: !is_archive_name(uri), state: ItemState::Waiting, nxm: None, from_collection: false }
    }
    fn link(m: u32, f: u32) -> NxmLink { NxmLink::parse(&format!("nxm://cyberpunk2077/mods/{m}/files/{f}?key=k&expires=9999999999")).unwrap() }

    #[test]
    fn free_queue_shows_the_cost_and_advances_on_links() {
        let mut q = Queue::default();
        let a = q.add(item(1, 10, "a.zip"));
        let b = q.add(item(2, 20, "b.7z"));
        assert_eq!(q.add(item(1, 10, "a.zip")), a, "no duplicates");
        let s = q.summary();
        assert_eq!((s.left, s.needs_click, s.bytes_left), (2, 2, 2000));
        assert_eq!(q.next_action(), Some(Action::OpenPage(a)));
        q.set(a, ItemState::NeedsClick);
        assert!(q.summary().waiting_on_user);

        q.accept_nxm(link(1, 10), |_| unreachable!());
        assert_eq!(q.next_action(), Some(Action::DownloadWith(a)));
        q.set(a, ItemState::Downloading { received: 400, total: Some(1000) });
        assert_eq!(q.next_action(), None, "one at a time");
        assert_eq!(q.summary().bytes_left, 1600);
        q.set(a, ItemState::Done { install_id: Some(1), note: None });
        assert_eq!(q.next_action(), Some(Action::OpenPage(b)));
        assert_eq!(q.summary().needs_click, 1);
    }

    #[test]
    fn premium_runs_without_clicks_except_manual_files() {
        let mut q = Queue { premium: true, ..Default::default() };
        let a = q.add(item(1, 10, "a.zip"));
        let m = q.add(item(2, 20, "Setup.exe"));
        assert_eq!(q.summary().needs_click, 1);
        assert_eq!(q.next_action(), Some(Action::FetchLink(a)));
        q.set(a, ItemState::Done { install_id: None, note: None });
        assert_eq!(q.next_action(), Some(Action::OpenPage(m)));
    }

    #[test]
    fn a_link_for_another_file_or_mod_is_taken() {
        let mut q = Queue::default();
        let a = q.add(item(1, 10, "a.zip"));
        assert_eq!(q.accept_nxm(link(1, 11), |_| unreachable!()), a);
        assert_eq!(q.get(a).unwrap().file_id, 11);
        let n = q.accept_nxm(link(7, 70), |l| item(l.mod_id, l.file_id, "x.zip"));
        assert_ne!(n, a);
        assert!(q.get(n).unwrap().nxm.is_some());
    }

    #[test]
    fn the_queue_survives_a_restart() {
        let db = Db::memory().unwrap();
        let mut q = Queue::default();
        let a = q.add(item(1, 10, "a.zip"));
        q.add(item(2, 20, "b.zip"));
        q.set(a, ItemState::Downloading { received: 5, total: Some(10) });
        q.accept_nxm(link(2, 20), |_| unreachable!());
        save(&db, &q).unwrap();
        let back = load(&db).unwrap();
        assert_eq!(back.items.len(), 2);
        assert!(back.items.iter().all(|i| i.state == ItemState::Waiting && i.nxm.is_none()));
        let mut back = back;
        assert!(back.add(item(3, 30, "c.zip")) > 2, "ids keep counting");
    }

    #[test]
    fn file_names_from_nexus() {
        assert!(is_archive_name("ArchiveXL-4198-1-28-6.zip"));
        assert!(!is_archive_name("Setup-1-0.exe"));
        assert!(is_archive_name("4c/ec/e3/4cece3d6-4ffb-4c5a-b4d0-12635932ae9e"), "storage path from a collection");
        assert!(is_archive_name("My.Mod.v1.2"), "dots in a name are not an extension");
        assert_eq!(download_name(1, 2, "4c/ec/e3/4cece3d6"), "1-2-4cece3d6");
        assert_eq!(download_name(1, 2, "A-1.7z"), "A-1.7z");
    }

    #[test]
    fn failures_and_retry() {
        let mut q = Queue::default();
        let a = q.add(item(1, 10, "a.zip"));
        q.set(a, ItemState::Failed { message: "x".into() });
        assert_eq!(q.summary().failed, 1);
        assert!(q.summary().waiting_on_user);
        q.retry(a);
        assert_eq!(q.get(a).unwrap().state, ItemState::Waiting);
    }
}
