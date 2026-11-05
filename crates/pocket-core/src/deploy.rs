//! Putting mod files into the game folder, and taking them out again, reversibly.
//!
//! Each mod's files live in the mod store (on the same drive as the game). Deploying hard-links them into the
//! game folder; no symbolic links (they need admin rights or Developer Mode) and no silent copies. A copy
//! fallback exists for drives that can't hard-link, and it is only used when the person turned it on.
//!
//! Every deploy writes a manifest: each file written, how, and what was there before. Undeploying walks the
//! manifest backwards and only removes a file when it is still this mod's file, so it never deletes a file
//! another mod (or the person) put there since. A file that was there before any mod is moved into a backup
//! folder and put back on undeploy. A file another mod owned is restored from that mod's store.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    HardLink,
    Copy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    /// Game-relative path, forward slashes, original case.
    pub rel: String,
    /// The file in the mod store this entry came from.
    pub source: PathBuf,
    pub method: Method,
    pub size: u64,
    /// A pre-existing file (owned by no mod) that was moved aside to make room.
    pub backup: Option<PathBuf>,
    /// The mod whose file this replaced; it is put back when this mod is undeployed.
    pub displaced: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub mod_key: String,
    pub game_root: PathBuf,
    pub entries: Vec<ManifestEntry>,
    /// Game-relative folders this deploy created, in creation order. Removed on undeploy when empty.
    pub created_dirs: Vec<String>,
    pub deployed_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct DeployOptions {
    /// Copy when hard links are impossible (different drive, FAT/exFAT). Off by default.
    pub allow_copy: bool,
    /// Replace files owned by other mods (this mod wins). When false those files are skipped and reported.
    pub overwrite_other_mods: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileConflict {
    pub rel: String,
    pub other_mod: String,
    /// True when this mod's file replaced the other's; false when it was skipped.
    pub won: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployReport {
    pub manifest: Manifest,
    pub conflicts: Vec<FileConflict>,
    pub copied: usize,
    pub backed_up: usize,
}

/// One file to deploy.
#[derive(Debug, Clone)]
pub struct PlannedFile {
    pub source: PathBuf,
    pub rel: String,
}

/// Who owns which game-relative path right now, from all deployed manifests.
#[derive(Debug, Default, Clone)]
pub struct Ownership {
    map: HashMap<String, (String, PathBuf)>,
}

pub fn key_of(rel: &str) -> String {
    rel.replace('\\', "/").to_lowercase()
}

impl Ownership {
    pub fn from_manifests(manifests: &[Manifest]) -> Ownership {
        let mut o = Ownership::default();
        for m in manifests {
            for e in &m.entries {
                o.map.insert(key_of(&e.rel), (m.mod_key.clone(), e.source.clone()));
            }
        }
        // a displaced owner is not the owner; the displacer is. Second pass so order doesn't matter.
        for m in manifests {
            for e in &m.entries {
                if e.displaced.is_some() {
                    o.map.insert(key_of(&e.rel), (m.mod_key.clone(), e.source.clone()));
                }
            }
        }
        o
    }
    pub fn owner(&self, rel: &str) -> Option<&str> {
        self.map.get(&key_of(rel)).map(|(k, _)| k.as_str())
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

pub fn game_path(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').filter(|p| !p.is_empty()).fold(root.to_path_buf(), |p, part| p.join(part))
}

/// Error codes that mean "these two paths can't be hard-linked".
fn is_link_unsupported(e: &std::io::Error) -> bool {
    match e.raw_os_error() {
        // Windows: ERROR_INVALID_FUNCTION (FAT), ERROR_NOT_SAME_DEVICE, ERROR_NOT_SUPPORTED
        #[cfg(windows)]
        Some(1) | Some(17) | Some(50) => true,
        // Unix: EXDEV, EPERM on some filesystems, ENOTSUP
        #[cfg(unix)]
        Some(18) | Some(95) => true,
        _ => matches!(e.kind(), std::io::ErrorKind::CrossesDevices | std::io::ErrorKind::Unsupported),
    }
}

fn move_file(from: &Path, to: &Path) -> Result<()> {
    if let Some(p) = to.parent() {
        fs::create_dir_all(p).map_err(|e| Error::io_at(e, p))?;
    }
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(from, to).map_err(|e| Error::io_at(e, to))?;
            fs::remove_file(from).map_err(|e| Error::io_at(e, from))
        }
    }
}

enum Done {
    Linked(PathBuf),
    MovedAside { original: PathBuf, backup: PathBuf },
    Unlinked { target: PathBuf, restore_from: PathBuf },
    Dir(PathBuf),
}

fn rollback(done: Vec<Done>) {
    for d in done.into_iter().rev() {
        let _ = match d {
            Done::Linked(t) => fs::remove_file(&t).map_err(Error::from),
            Done::MovedAside { original, backup } => move_file(&backup, &original),
            Done::Unlinked { target, restore_from } => fs::hard_link(&restore_from, &target).or_else(|_| fs::copy(&restore_from, &target).map(|_| ())).map_err(Error::from),
            Done::Dir(d) => fs::remove_dir(&d).map_err(Error::from),
        };
    }
}

/// Deploys `files` for `mod_key` into `game_root`. On any failure everything done so far is undone.
pub fn deploy(
    mod_key: &str,
    game_root: &Path,
    files: &[PlannedFile],
    owners: &Ownership,
    backup_root: &Path,
    opts: &DeployOptions,
) -> Result<DeployReport> {
    let mut done: Vec<Done> = Vec::new();
    let mut manifest = Manifest {
        mod_key: mod_key.into(),
        game_root: game_root.to_path_buf(),
        entries: Vec::new(),
        created_dirs: Vec::new(),
        deployed_at: crate::timefmt::now(),
    };
    let mut conflicts = Vec::new();
    let (mut copied, mut backed_up) = (0, 0);

    let result = (|| -> Result<()> {
        for f in files {
            let target = game_path(game_root, &f.rel);
            let size = fs::metadata(&f.source).map_err(|e| Error::io_at(e, &f.source))?.len();
            let mut entry = ManifestEntry { rel: f.rel.clone(), source: f.source.clone(), method: Method::HardLink, size, backup: None, displaced: None };

            match owners.owner(&f.rel) {
                Some(other) if other == mod_key => {
                    // a redeploy of the same mod: replace our own old link
                    if target.exists() {
                        let (_, old_source) = owners.map.get(&key_of(&f.rel)).cloned().unwrap();
                        fs::remove_file(&target).map_err(|e| Error::io_at(e, &target))?;
                        done.push(Done::Unlinked { target: target.clone(), restore_from: old_source });
                    }
                }
                Some(other) => {
                    if !opts.overwrite_other_mods {
                        conflicts.push(FileConflict { rel: f.rel.clone(), other_mod: other.into(), won: false });
                        continue;
                    }
                    let (_, other_source) = owners.map.get(&key_of(&f.rel)).cloned().unwrap();
                    if target.exists() {
                        fs::remove_file(&target).map_err(|e| Error::io_at(e, &target))?;
                        done.push(Done::Unlinked { target: target.clone(), restore_from: other_source });
                    }
                    entry.displaced = Some(other.into());
                    conflicts.push(FileConflict { rel: f.rel.clone(), other_mod: other.into(), won: true });
                }
                None if target.exists() => {
                    let backup = game_path(&backup_root.join(mod_key), &f.rel);
                    move_file(&target, &backup)?;
                    done.push(Done::MovedAside { original: target.clone(), backup: backup.clone() });
                    entry.backup = Some(backup);
                    backed_up += 1;
                }
                None => {}
            }

            // create missing parent folders one level at a time, so each can be removed again
            if let Some(parent) = target.parent() {
                let mut missing = Vec::new();
                let mut p = parent.to_path_buf();
                while !p.exists() && p.starts_with(game_root) && p != game_root {
                    missing.push(p.clone());
                    if !p.pop() {
                        break;
                    }
                }
                for d in missing.into_iter().rev() {
                    fs::create_dir(&d).map_err(|e| Error::io_at(e, &d))?;
                    let rel = d.strip_prefix(game_root).unwrap_or(&d).to_string_lossy().replace('\\', "/");
                    manifest.created_dirs.push(rel);
                    done.push(Done::Dir(d));
                }
            }

            match fs::hard_link(&f.source, &target) {
                Ok(()) => {}
                Err(e) if is_link_unsupported(&e) => {
                    if !opts.allow_copy {
                        return Err(Error::Other(format!(
                            "Couldn't hard-link into the game folder ({e}). Keep the mod store on the same drive as the game, \
                             or turn on \"Copy when hard links fail\" in Settings."
                        )));
                    }
                    fs::copy(&f.source, &target).map_err(|e| Error::io_at(e, &target))?;
                    entry.method = Method::Copy;
                    copied += 1;
                }
                Err(e) => return Err(Error::io_at(e, &target)),
            }
            done.push(Done::Linked(target));
            manifest.entries.push(entry);
        }
        Ok(())
    })();

    match result {
        Ok(()) => Ok(DeployReport { manifest, conflicts, copied, backed_up }),
        Err(e) => {
            rollback(done);
            Err(e)
        }
    }
}

/// Is the file at `target` still the one this entry put there?
fn still_ours(entry: &ManifestEntry, target: &Path) -> bool {
    match entry.method {
        Method::HardLink => same_file::is_same_file(target, &entry.source).unwrap_or(false),
        Method::Copy => fs::metadata(target).map(|m| m.len() == entry.size).unwrap_or(false),
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndeployReport {
    pub removed: usize,
    pub restored_backups: usize,
    pub restored_other_mods: usize,
    /// Files changed by someone else since the deploy; left alone.
    pub left_alone: Vec<String>,
}

/// Takes a mod's files out of the game folder. `others` are the other deployed mods' manifests; ones that
/// displaced this mod's files inherit what this mod had displaced or backed up, so nothing is lost.
pub fn undeploy(manifest: &Manifest, others: &mut [Manifest]) -> Result<UndeployReport> {
    let mut rep = UndeployReport::default();
    let root = &manifest.game_root;
    for e in manifest.entries.iter().rev() {
        let target = game_path(root, &e.rel);
        // did another mod displace us here? then hand our history to it and leave its file alone
        if let Some(heir) = others.iter_mut().flat_map(|m| m.entries.iter_mut()).find(|o| key_of(&o.rel) == key_of(&e.rel) && o.displaced.as_deref() == Some(&manifest.mod_key)) {
            heir.displaced = e.displaced.clone();
            heir.backup = e.backup.clone();
            continue;
        }
        if target.exists() {
            if !still_ours(e, &target) {
                rep.left_alone.push(e.rel.clone());
                continue;
            }
            fs::remove_file(&target).map_err(|err| Error::io_at(err, &target))?;
            rep.removed += 1;
        }
        if let Some(prev) = &e.displaced {
            // put the displaced mod's file back, if that mod is still deployed
            if let Some(src) = others.iter().filter(|m| &m.mod_key == prev).flat_map(|m| m.entries.iter()).find(|o| key_of(&o.rel) == key_of(&e.rel)).map(|o| o.source.clone()) {
                if !target.exists() {
                    fs::hard_link(&src, &target).or_else(|_| fs::copy(&src, &target).map(|_| ())).map_err(|err| Error::io_at(err, &target))?;
                    rep.restored_other_mods += 1;
                }
            }
        } else if let Some(b) = &e.backup {
            if b.exists() && !target.exists() {
                move_file(b, &target)?;
                rep.restored_backups += 1;
            }
        }
    }
    for d in manifest.created_dirs.iter().rev() {
        let p = game_path(root, d);
        // only removes empty folders; a folder something else wrote into stays
        let _ = fs::remove_dir(&p);
    }
    Ok(rep)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drift {
    pub rel: String,
    /// "missing" or "changed"
    pub state: String,
}

/// Files of a deployed mod that are gone or were replaced outside the app.
pub fn verify(manifest: &Manifest) -> Vec<Drift> {
    manifest
        .entries
        .iter()
        .filter_map(|e| {
            let t = game_path(&manifest.game_root, &e.rel);
            if !t.exists() {
                Some(Drift { rel: e.rel.clone(), state: "missing".into() })
            } else if e.displaced.is_none() && !still_ours(e, &t) {
                Some(Drift { rel: e.rel.clone(), state: "changed".into() })
            } else {
                None
            }
        })
        .collect()
}

/// The mod store: staged files, manifests and backups, kept next to each other on the game's drive.
#[derive(Debug, Clone)]
pub struct Store {
    pub root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Store {
        Store { root: root.into() }
    }
    pub fn staging(&self, mod_key: &str) -> PathBuf {
        self.root.join("mods").join(mod_key)
    }
    pub fn downloads(&self) -> PathBuf {
        self.root.join("downloads")
    }
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }
    fn manifests_dir(&self) -> PathBuf {
        self.root.join("manifests")
    }
    pub fn manifest_path(&self, mod_key: &str) -> PathBuf {
        self.manifests_dir().join(format!("{mod_key}.json"))
    }
    pub fn save_manifest(&self, m: &Manifest) -> Result<()> {
        let dir = self.manifests_dir();
        fs::create_dir_all(&dir).map_err(|e| Error::io_at(e, &dir))?;
        let path = self.manifest_path(&m.mod_key);
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(m)?).map_err(|e| Error::io_at(e, &tmp))?;
        fs::rename(&tmp, &path).map_err(|e| Error::io_at(e, &path))
    }
    pub fn delete_manifest(&self, mod_key: &str) -> Result<()> {
        let p = self.manifest_path(mod_key);
        if p.exists() {
            fs::remove_file(&p).map_err(|e| Error::io_at(e, &p))?;
        }
        Ok(())
    }
    pub fn load_manifest(&self, mod_key: &str) -> Result<Option<Manifest>> {
        let p = self.manifest_path(mod_key);
        if !p.exists() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_slice(&fs::read(&p).map_err(|e| Error::io_at(e, &p))?)?))
    }
    pub fn load_manifests(&self) -> Result<Vec<Manifest>> {
        let dir = self.manifests_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for e in fs::read_dir(&dir).map_err(|e| Error::io_at(e, &dir))? {
            let p = e?.path();
            if p.extension().and_then(|x| x.to_str()) == Some("json") {
                out.push(serde_json::from_slice(&fs::read(&p)?)?);
            }
        }
        out.sort_by_key(|m: &Manifest| m.deployed_at);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _dir: tempfile::TempDir,
        game: PathBuf,
        store: Store,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("Cyberpunk 2077");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        fs::write(game.join("bin/x64/Cyberpunk2077.exe"), b"game").unwrap();
        let store = Store::new(dir.path().join("store"));
        Fixture { _dir: dir, game, store }
    }

    fn stage(fx: &Fixture, key: &str, files: &[(&str, &str)]) -> Vec<PlannedFile> {
        files
            .iter()
            .map(|(rel, body)| {
                let src = game_path(&fx.store.staging(key), rel);
                fs::create_dir_all(src.parent().unwrap()).unwrap();
                fs::write(&src, body).unwrap();
                PlannedFile { source: src, rel: rel.to_string() }
            })
            .collect()
    }

    fn read(fx: &Fixture, rel: &str) -> Option<String> {
        fs::read_to_string(game_path(&fx.game, rel)).ok()
    }

    #[test]
    fn deploy_and_undeploy_leave_the_game_as_it_was() {
        let fx = fixture();
        let files = stage(&fx, "a", &[("archive/pc/mod/a.archive", "A"), ("r6/scripts/a/a.reds", "AR")]);
        let rep = deploy("a", &fx.game, &files, &Ownership::default(), &fx.store.backups(), &DeployOptions::default()).unwrap();
        assert_eq!(rep.manifest.entries.len(), 2);
        assert_eq!(rep.manifest.entries[0].method, Method::HardLink);
        assert!(same_file::is_same_file(game_path(&fx.game, "archive/pc/mod/a.archive"), &files[0].source).unwrap());
        assert_eq!(rep.manifest.created_dirs, vec!["archive", "archive/pc", "archive/pc/mod", "r6", "r6/scripts", "r6/scripts/a"]);

        let un = undeploy(&rep.manifest, &mut []).unwrap();
        assert_eq!(un.removed, 2);
        assert!(!fx.game.join("archive").exists(), "created folders are removed");
        assert!(!fx.game.join("r6").exists());
        assert_eq!(read(&fx, "bin/x64/Cyberpunk2077.exe").as_deref(), Some("game"));
        assert!(files[0].source.exists(), "the store keeps its copy");
    }

    #[test]
    fn pre_existing_files_are_backed_up_and_restored() {
        let fx = fixture();
        fs::create_dir_all(fx.game.join("engine/config")).unwrap();
        fs::write(fx.game.join("engine/config/x.ini"), "original").unwrap();
        let files = stage(&fx, "a", &[("engine/config/x.ini", "modded")]);
        let rep = deploy("a", &fx.game, &files, &Ownership::default(), &fx.store.backups(), &DeployOptions::default()).unwrap();
        assert_eq!(rep.backed_up, 1);
        assert_eq!(read(&fx, "engine/config/x.ini").as_deref(), Some("modded"));
        undeploy(&rep.manifest, &mut []).unwrap();
        assert_eq!(read(&fx, "engine/config/x.ini").as_deref(), Some("original"));
        assert!(fx.game.join("engine/config").exists(), "folders that were there before stay");
    }

    #[test]
    fn conflicts_skip_by_default_and_win_when_asked() {
        let fx = fixture();
        let a = stage(&fx, "a", &[("archive/pc/mod/shared.archive", "A")]);
        let ra = deploy("a", &fx.game, &a, &Ownership::default(), &fx.store.backups(), &DeployOptions::default()).unwrap();
        let owners = Ownership::from_manifests(std::slice::from_ref(&ra.manifest));
        assert_eq!(owners.owner("ARCHIVE/pc/mod/shared.archive"), Some("a"));

        let b = stage(&fx, "b", &[("archive/pc/mod/shared.archive", "B")]);
        let skip = deploy("b", &fx.game, &b, &owners, &fx.store.backups(), &DeployOptions::default()).unwrap();
        assert!(skip.manifest.entries.is_empty());
        assert!(!skip.conflicts[0].won);
        assert_eq!(read(&fx, "archive/pc/mod/shared.archive").as_deref(), Some("A"));

        let opts = DeployOptions { overwrite_other_mods: true, ..Default::default() };
        let rb = deploy("b", &fx.game, &b, &owners, &fx.store.backups(), &opts).unwrap();
        assert!(rb.conflicts[0].won);
        assert_eq!(read(&fx, "archive/pc/mod/shared.archive").as_deref(), Some("B"));
        let owners = Ownership::from_manifests(&[ra.manifest.clone(), rb.manifest.clone()]);
        assert_eq!(owners.owner("archive/pc/mod/shared.archive"), Some("b"));

        // removing b gives the file back to a
        let mut others = vec![ra.manifest.clone()];
        let un = undeploy(&rb.manifest, &mut others).unwrap();
        assert_eq!(un.restored_other_mods, 1);
        assert_eq!(read(&fx, "archive/pc/mod/shared.archive").as_deref(), Some("A"));
    }

    #[test]
    fn removing_the_displaced_mod_never_deletes_the_winners_file() {
        let fx = fixture();
        fs::create_dir_all(fx.game.join("archive/pc/mod")).unwrap();
        fs::write(fx.game.join("archive/pc/mod/s.archive"), "vanilla").unwrap();
        let a = stage(&fx, "a", &[("archive/pc/mod/s.archive", "A")]);
        let ra = deploy("a", &fx.game, &a, &Ownership::default(), &fx.store.backups(), &DeployOptions::default()).unwrap();
        let b = stage(&fx, "b", &[("archive/pc/mod/s.archive", "B")]);
        let opts = DeployOptions { overwrite_other_mods: true, ..Default::default() };
        let rb = deploy("b", &fx.game, &b, &Ownership::from_manifests(std::slice::from_ref(&ra.manifest)), &fx.store.backups(), &opts).unwrap();

        // undeploy a first: b keeps its file and inherits a's backup of the vanilla file
        let mut others = vec![rb.manifest.clone()];
        undeploy(&ra.manifest, &mut others).unwrap();
        assert_eq!(read(&fx, "archive/pc/mod/s.archive").as_deref(), Some("B"));
        let rb2 = others.pop().unwrap();
        assert!(rb2.entries[0].backup.is_some() && rb2.entries[0].displaced.is_none());

        // then b: the vanilla file comes back
        undeploy(&rb2, &mut []).unwrap();
        assert_eq!(read(&fx, "archive/pc/mod/s.archive").as_deref(), Some("vanilla"));
    }

    #[test]
    fn files_changed_outside_are_left_alone() {
        let fx = fixture();
        let a = stage(&fx, "a", &[("archive/pc/mod/a.archive", "A")]);
        let ra = deploy("a", &fx.game, &a, &Ownership::default(), &fx.store.backups(), &DeployOptions::default()).unwrap();
        let t = game_path(&fx.game, "archive/pc/mod/a.archive");
        fs::remove_file(&t).unwrap();
        fs::write(&t, "someone else's").unwrap();
        assert_eq!(verify(&ra.manifest)[0].state, "changed");
        let un = undeploy(&ra.manifest, &mut []).unwrap();
        assert_eq!(un.left_alone, vec!["archive/pc/mod/a.archive"]);
        assert_eq!(read(&fx, "archive/pc/mod/a.archive").as_deref(), Some("someone else's"));
    }

    #[test]
    fn a_failed_deploy_rolls_back() {
        let fx = fixture();
        let mut files = stage(&fx, "a", &[("archive/pc/mod/a.archive", "A")]);
        files.push(PlannedFile { source: fx.store.staging("a").join("does-not-exist"), rel: "archive/pc/mod/b.archive".into() });
        assert!(deploy("a", &fx.game, &files, &Ownership::default(), &fx.store.backups(), &DeployOptions::default()).is_err());
        assert!(!fx.game.join("archive").exists(), "nothing is left behind");
    }

    #[test]
    fn manifests_persist() {
        let fx = fixture();
        let a = stage(&fx, "a", &[("archive/pc/mod/a.archive", "A")]);
        let ra = deploy("a", &fx.game, &a, &Ownership::default(), &fx.store.backups(), &DeployOptions::default()).unwrap();
        fx.store.save_manifest(&ra.manifest).unwrap();
        assert_eq!(fx.store.load_manifests().unwrap(), vec![ra.manifest.clone()]);
        assert_eq!(fx.store.load_manifest("a").unwrap(), Some(ra.manifest));
        fx.store.delete_manifest("a").unwrap();
        assert!(fx.store.load_manifests().unwrap().is_empty());
    }
}
