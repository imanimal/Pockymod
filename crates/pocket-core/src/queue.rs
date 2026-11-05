//! The download queue. Pure state: the Tauri shell drives it (resolving links, downloading, installing) and
//! shows its summary on the Downloads LCDs.
//!
//! Premium accounts: every item resolves its own download link, so the queue runs on its own.
//! Free accounts: each item needs a click on Nexus's "Mod Manager Download" button, which produces an nxm://
//! link. The queue says up front how many clicks it needs, opens the next item's page inside the app, catches
//! the link and moves on by itself. Mods marked manual-only always go through their page.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemState {
    /// Not started.
    Waiting,
    /// Its Nexus page is open, waiting for the person's click.
    NeedsClick,
    /// Asking Nexus for the download link.
    Resolving,
    Downloading,
    Installing,
    Done,
    Failed,
}

impl ItemState {
    pub fn is_active(self) -> bool {
        matches!(self, ItemState::Resolving | ItemState::Downloading | ItemState::Installing)
    }
    pub fn is_pending(self) -> bool {
        matches!(self, ItemState::Waiting | ItemState::NeedsClick)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: u64,
    pub mod_id: u64,
    pub file_id: u64,
    pub mod_name: String,
    pub file_name: String,
    pub version: Option<String>,
    pub size_bytes: u64,
    pub received: u64,
    pub state: ItemState,
    pub error: Option<String>,
    /// The mod's page says download by hand: always goes through its page, even for Premium.
    pub manual_only: bool,
    /// "browse", "update", "dependency", "collection:<slug>"
    pub origin: String,
    pub added_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NewItem {
    pub mod_id: u64,
    pub file_id: u64,
    pub mod_name: String,
    pub file_name: String,
    pub version: Option<String>,
    pub size_bytes: u64,
    pub manual_only: bool,
    pub origin: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueueSummary {
    /// Items not finished yet (pending or active).
    pub left: usize,
    /// Of those, how many need a click on Nexus.
    pub needs_click: usize,
    pub bytes_left: u64,
    /// Seconds of downloading left at the measured speed; None before there's a measurement.
    pub eta_secs: Option<u64>,
    /// True when nothing can move until the person clicks (amber LCDs).
    pub waiting_on_user: bool,
    pub running: bool,
    pub done: usize,
    pub failed: usize,
    pub active: Option<u64>,
    /// Overall progress 0..=100 for the program key's foot bar.
    pub percent: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "step", content = "id", rename_all = "camelCase")]
pub enum Step {
    /// Nothing to do.
    Idle,
    /// Something is in flight; wait for it.
    Busy,
    /// Premium: resolve this item's link and download it.
    Resolve(u64),
    /// Free (or manual-only): open this item's page and wait for the click.
    AskClick(u64),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Queue {
    pub items: Vec<QueueItem>,
    pub premium: bool,
    pub paused: bool,
    next_id: u64,
    /// Bytes per second, smoothed.
    speed: Option<f64>,
}

impl Queue {
    pub fn new(premium: bool) -> Queue {
        Queue { premium, next_id: 1, ..Default::default() }
    }

    /// Adds an item unless the same file is already queued and not finished. Returns its id either way.
    pub fn enqueue(&mut self, n: NewItem, now: i64) -> u64 {
        if let Some(existing) = self.items.iter_mut().find(|i| i.mod_id == n.mod_id && i.file_id == n.file_id && i.state != ItemState::Done) {
            if existing.state == ItemState::Failed {
                existing.state = ItemState::Waiting;
                existing.error = None;
            }
            return existing.id;
        }
        self.next_id = self.next_id.max(1);
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(QueueItem {
            id,
            mod_id: n.mod_id,
            file_id: n.file_id,
            mod_name: n.mod_name,
            file_name: n.file_name,
            version: n.version,
            size_bytes: n.size_bytes,
            received: 0,
            state: ItemState::Waiting,
            error: None,
            manual_only: n.manual_only,
            origin: n.origin,
            added_at: now,
        });
        id
    }

    pub fn get(&self, id: u64) -> Option<&QueueItem> {
        self.items.iter().find(|i| i.id == id)
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut QueueItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    fn needs_click(&self, i: &QueueItem) -> bool {
        !self.premium || i.manual_only
    }

    /// What the driver should do next. One item is in flight at a time, which keeps request pacing simple.
    pub fn next_step(&mut self) -> Step {
        if self.items.iter().any(|i| i.state.is_active()) {
            return Step::Busy;
        }
        if self.paused {
            return Step::Idle;
        }
        let premium = self.premium;
        let Some(item) = self.items.iter_mut().find(|i| i.state.is_pending()) else { return Step::Idle };
        if premium && !item.manual_only {
            item.state = ItemState::Resolving;
            Step::Resolve(item.id)
        } else {
            item.state = ItemState::NeedsClick;
            Step::AskClick(item.id)
        }
    }

    /// An nxm link arrived for (mod, file). Matches the item waiting for it; a different file of the same mod
    /// (the person picked another file on the page) takes that item's place. Returns the item, or None when
    /// nothing in the queue asked for it.
    pub fn on_nxm(&mut self, mod_id: u64, file_id: u64) -> Option<u64> {
        let pos = self
            .items
            .iter()
            .position(|i| i.mod_id == mod_id && i.file_id == file_id && i.state.is_pending())
            .or_else(|| self.items.iter().position(|i| i.mod_id == mod_id && i.state == ItemState::NeedsClick))?;
        let item = &mut self.items[pos];
        item.file_id = file_id;
        item.state = ItemState::Resolving;
        Some(item.id)
    }

    pub fn set_state(&mut self, id: u64, state: ItemState) {
        if let Some(i) = self.get_mut(id) {
            i.state = state;
            if state != ItemState::Failed {
                i.error = None;
            }
        }
    }

    /// Download progress. `elapsed` is the seconds since the previous update, for the speed estimate.
    pub fn progress(&mut self, id: u64, received: u64, total: Option<u64>, delta_bytes: u64, elapsed: f64) {
        if let Some(i) = self.get_mut(id) {
            i.state = ItemState::Downloading;
            i.received = received;
            if let Some(t) = total.filter(|t| *t > 0) {
                i.size_bytes = t;
            }
        }
        if elapsed > 0.05 && delta_bytes > 0 {
            let sample = delta_bytes as f64 / elapsed;
            self.speed = Some(match self.speed {
                Some(s) => s * 0.8 + sample * 0.2,
                None => sample,
            });
        }
    }

    pub fn fail(&mut self, id: u64, error: impl Into<String>) {
        if let Some(i) = self.get_mut(id) {
            i.state = ItemState::Failed;
            i.error = Some(error.into());
        }
    }

    pub fn retry(&mut self, id: u64) {
        if let Some(i) = self.get_mut(id) {
            if matches!(i.state, ItemState::Failed | ItemState::NeedsClick) {
                i.state = ItemState::Waiting;
                i.error = None;
                i.received = 0;
            }
        }
    }

    /// Puts a NeedsClick item back to waiting and moves it to the end (the person skipped it for now).
    pub fn skip(&mut self, id: u64) {
        if let Some(pos) = self.items.iter().position(|i| i.id == id && i.state.is_pending()) {
            let mut it = self.items.remove(pos);
            it.state = ItemState::Waiting;
            self.items.push(it);
        }
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id || i.state.is_active());
        before != self.items.len()
    }

    pub fn clear_finished(&mut self) {
        self.items.retain(|i| !matches!(i.state, ItemState::Done));
    }

    pub fn summary(&self) -> QueueSummary {
        let unfinished: Vec<&QueueItem> = self.items.iter().filter(|i| i.state.is_pending() || i.state.is_active()).collect();
        let needs_click = unfinished.iter().filter(|i| i.state.is_pending() && self.needs_click(i)).count();
        let bytes_left: u64 = unfinished.iter().map(|i| i.size_bytes.saturating_sub(i.received)).sum();
        let total: u64 = self.items.iter().filter(|i| i.state != ItemState::Failed).map(|i| i.size_bytes.max(1)).sum();
        let got: u64 = self
            .items
            .iter()
            .filter(|i| i.state != ItemState::Failed)
            .map(|i| if i.state == ItemState::Done { i.size_bytes.max(1) } else { i.received.min(i.size_bytes.max(1)) })
            .sum();
        let active = self.items.iter().find(|i| i.state.is_active()).map(|i| i.id);
        let waiting_on_user = active.is_none() && self.items.iter().any(|i| i.state == ItemState::NeedsClick);
        QueueSummary {
            left: unfinished.len(),
            needs_click,
            bytes_left,
            eta_secs: self.speed.filter(|s| *s > 1.0).map(|s| (bytes_left as f64 / s).ceil() as u64),
            waiting_on_user,
            running: active.is_some(),
            done: self.items.iter().filter(|i| i.state == ItemState::Done).count(),
            failed: self.items.iter().filter(|i| i.state == ItemState::Failed).count(),
            active,
            percent: if total == 0 { 0 } else { ((got as f64 / total as f64) * 100.0).round().min(100.0) as u8 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(mod_id: u64, file_id: u64) -> NewItem {
        NewItem { mod_id, file_id, mod_name: format!("Mod {mod_id}"), file_name: "f.zip".into(), version: None, size_bytes: 1000, manual_only: false, origin: "browse".into() }
    }

    #[test]
    fn free_queue_shows_cost_and_waits_for_clicks() {
        let mut q = Queue::new(false);
        for m in 1..=7 {
            q.enqueue(item(m, m * 10), 0);
        }
        let s = q.summary();
        assert_eq!((s.left, s.needs_click), (7, 7), "7 mods need a click on Nexus");
        assert_eq!(q.next_step(), Step::AskClick(1));
        assert!(q.summary().waiting_on_user);
        // asking again while waiting asks for the same item, not the next
        assert_eq!(q.next_step(), Step::AskClick(1));
        // the click arrives
        assert_eq!(q.on_nxm(1, 10), Some(1));
        assert_eq!(q.next_step(), Step::Busy);
        q.progress(1, 500, Some(1000), 500, 1.0);
        assert!(q.summary().eta_secs.is_some());
        q.set_state(1, ItemState::Installing);
        q.set_state(1, ItemState::Done);
        assert_eq!(q.next_step(), Step::AskClick(2), "advances by itself");
        assert_eq!(q.summary().needs_click, 6);
    }

    #[test]
    fn premium_runs_alone_except_manual_only() {
        let mut q = Queue::new(true);
        q.enqueue(item(1, 1), 0);
        let mut manual = item(2, 2);
        manual.manual_only = true;
        q.enqueue(manual, 0);
        assert_eq!(q.summary().needs_click, 1);
        assert_eq!(q.next_step(), Step::Resolve(1));
        q.set_state(1, ItemState::Done);
        assert_eq!(q.next_step(), Step::AskClick(2));
    }

    #[test]
    fn nxm_for_another_file_of_the_waiting_mod_replaces_it() {
        let mut q = Queue::new(false);
        q.enqueue(item(5, 50), 0);
        q.next_step();
        assert_eq!(q.on_nxm(5, 51), Some(1));
        assert_eq!(q.get(1).unwrap().file_id, 51);
        assert_eq!(q.on_nxm(9, 90), None, "unrelated links are not swallowed");
    }

    #[test]
    fn dedupe_retry_skip_remove() {
        let mut q = Queue::new(false);
        let a = q.enqueue(item(1, 1), 0);
        assert_eq!(q.enqueue(item(1, 1), 0), a);
        q.fail(a, "network");
        assert_eq!(q.summary().failed, 1);
        assert_eq!(q.enqueue(item(1, 1), 0), a, "re-adding a failed item retries it");
        assert_eq!(q.get(a).unwrap().state, ItemState::Waiting);
        let b = q.enqueue(item(2, 2), 0);
        q.skip(a);
        assert_eq!(q.items.last().unwrap().id, a);
        assert!(q.remove(b));
        assert_eq!(q.items.len(), 1);
    }

    #[test]
    fn paused_queue_is_idle() {
        let mut q = Queue::new(true);
        q.enqueue(item(1, 1), 0);
        q.paused = true;
        assert_eq!(q.next_step(), Step::Idle);
    }

    #[test]
    fn percent_counts_done_items() {
        let mut q = Queue::new(true);
        q.enqueue(item(1, 1), 0);
        q.enqueue(item(2, 2), 0);
        q.set_state(1, ItemState::Done);
        assert_eq!(q.summary().percent, 50);
    }
}
