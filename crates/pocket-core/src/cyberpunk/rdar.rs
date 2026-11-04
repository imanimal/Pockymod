//! Cyberpunk 2077 `.archive` files (RDAR): which game resources each one holds, read from the header and the file
//! table only (never the file data), so archives that change the same game files can be found quickly.
//!
//! Layout, little-endian, as WolvenKit reads and writes it (`WolvenKit.RED4/Archive/IO/ArchiveReader.cs` and
//! `ArchiveWriter.cs` in the WolvenKit repository):
//!
//! ```text
//! header, at 0, 40 bytes
//!   u32 magic "RDAR" (0x52414452) | u32 version (12) | u64 index_position | u32 index_size
//!   u64 debug_position | u32 debug_size | u64 file_size
//! u32 custom_data_length at 40 (WolvenKit pads to 0xAC and may put a block of custom resource paths there)
//! index, at index_position, index_size bytes
//!   u32 file_table_offset (8) | u32 file_table_size | u64 crc64 of what follows
//!   u32 file_entry_count | u32 file_segment_count | u32 resource_dependency_count
//!   file_entry_count entries of 56 bytes:
//!     u64 name_hash (FNV-1a 64 of the resource path, such as base\characters\...\x.mesh)
//!     i64 timestamp (FILETIME) | u32 inline_buffer_segments | u32 segments_start | u32 segments_end
//!     u32 dependencies_start | u32 dependencies_end | [u8; 20] sha1 of the file
//!   then the segments (16 bytes each) and the dependency hashes (8 bytes each)
//! ```
//!
//! Only each entry's name hash (which resource) and SHA-1 (its content) are kept. Two archives with the same hash
//! and the same SHA-1 carry the very same file, which changes nothing in game.
//!
//! Load order, from the modding wiki's load order page (wiki.redmodding.org, for mod users):
//! - `.archive` files directly in `archive/pc/mod` load first, in ASCII order of their file names (`!` before
//!   `A`, `M` before `m`).
//! - Then REDmods' archives, `mods/<folder>/archives/*.archive`, by folder name in ASCII order.
//! - A `modlist.txt` in `archive/pc/mod` (one archive name per line) makes the game use that order instead; the
//!   wiki's advice is to delete it.
//! - When two archives hold the same resource, the one loaded first wins.
//!
//! UNVERIFIED: where archives that a `modlist.txt` does not name go (taken here as after the named ones, by name);
//! archives in subfolders of `archive/pc/mod` are left out.

use crate::db::Db;
use crate::error::{Error, Result};
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

pub const MAGIC: u32 = 0x5241_4452;
const HEADER: usize = 40;
const INDEX_HEAD: usize = 28;
const ENTRY: usize = 56;

/// One resource in an archive: its path hash and the SHA-1 of its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry { pub hash: u64, pub sha1: [u8; 20] }

/// FNV-1a 64, the hash archives use for resource paths.
pub fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() { h ^= b as u64; h = h.wrapping_mul(0x0000_0100_0000_01b3); }
    h
}

fn u32_at(b: &[u8], at: usize) -> u32 { u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) }
fn u64_at(b: &[u8], at: usize) -> u64 { u64::from_le_bytes(b[at..at + 8].try_into().unwrap()) }

/// Read an archive's file table.
pub fn read_entries(path: &Path) -> Result<Vec<Entry>> {
    let bad = |why: &str| Error::Invalid(format!("{} is not a readable archive: {why}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let mut f = fs::File::open(path)?;
    let len = f.metadata()?.len();
    let mut head = [0u8; HEADER];
    f.read_exact(&mut head).map_err(|_| bad("too short"))?;
    if u32_at(&head, 0) != MAGIC { return Err(bad("no RDAR header")); }
    let (index_pos, index_size) = (u64_at(&head, 8), u32_at(&head, 16) as u64);
    if index_size < INDEX_HEAD as u64 || index_pos.checked_add(index_size).is_none_or(|end| end > len) { return Err(bad("the file table is outside the file")); }
    f.seek(SeekFrom::Start(index_pos))?;
    let mut ih = [0u8; INDEX_HEAD];
    f.read_exact(&mut ih)?;
    let count = u32_at(&ih, 16) as u64;
    if count * ENTRY as u64 + INDEX_HEAD as u64 > index_size { return Err(bad("the file table is shorter than its count")); }
    let mut table = vec![0u8; (count as usize) * ENTRY];
    f.read_exact(&mut table)?;
    Ok(table.chunks_exact(ENTRY).map(|e| Entry { hash: u64_at(e, 0), sha1: e[36..56].try_into().unwrap() }).collect())
}

type CacheMap = HashMap<PathBuf, (u64, Option<SystemTime>, Arc<Vec<Entry>>)>;
static CACHE: OnceLock<Mutex<CacheMap>> = OnceLock::new();

/// `read_entries`, remembered while the file's size and modification time stay the same.
pub fn entries_cached(path: &Path) -> Result<Arc<Vec<Entry>>> {
    let m = fs::metadata(path)?;
    let (size, mtime) = (m.len(), m.modified().ok());
    let cache = CACHE.get_or_init(Default::default);
    if let Some((s, t, v)) = cache.lock().unwrap().get(path) {
        if *s == size && *t == mtime { return Ok(v.clone()); }
    }
    let v = Arc::new(read_entries(path)?);
    let mut c = cache.lock().unwrap();
    if c.len() > 4000 { c.clear(); }
    c.insert(path.to_path_buf(), (size, mtime, v.clone()));
    Ok(v)
}

// ── load order ──

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoadedArchive {
    /// Game-relative, forward slashes, as on disk.
    pub target: String,
    pub name: String,
    /// The REDmod folder, for REDmods' archives.
    pub redmod: Option<String>,
}

pub const MODLIST: &str = "archive/pc/mod/modlist.txt";

/// The names a `modlist.txt` lists, when there is one.
pub fn modlist(game: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(game.join(MODLIST)).ok()?;
    Some(text.lines().map(|l| l.trim().trim_start_matches('\u{feff}').to_string()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect())
}

fn archives_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir).into_iter().flatten().flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file() || t.is_symlink()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_lowercase().ends_with(".archive"))
        .collect();
    v.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    v
}

/// Every mod archive the game loads, first to last.
pub fn load_order(game: &Path) -> Vec<LoadedArchive> {
    let mut names = archives_in(&game.join("archive/pc/mod"));
    if let Some(list) = modlist(game) {
        let mut ordered = Vec::new();
        for l in &list {
            if let Some(i) = names.iter().position(|n| n.eq_ignore_ascii_case(l)) { ordered.push(names.remove(i)); }
        }
        ordered.append(&mut names);
        names = ordered;
    }
    let mut out: Vec<LoadedArchive> = names.into_iter().map(|n| LoadedArchive { target: format!("archive/pc/mod/{n}"), name: n, redmod: None }).collect();
    let mut folders: Vec<String> = fs::read_dir(game.join("mods")).into_iter().flatten().flatten()
        .filter(|e| e.path().join("archives").is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    folders.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    for f in folders {
        for n in archives_in(&game.join("mods").join(&f).join("archives")) {
            out.push(LoadedArchive { target: format!("mods/{f}/archives/{n}"), name: n, redmod: Some(f.clone()) });
        }
    }
    out
}

// ── overlaps ──

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Side {
    pub install_id: Option<i64>,
    pub mod_id: Option<u32>,
    pub name: String,
    /// The archives involved, game-relative.
    pub archives: Vec<String>,
    pub redmod: bool,
}

/// Rename these archives of an install (target as the mod's plan names it, new file name) so it loads first.
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenameTo { pub target: String, pub name: String }

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Overlap {
    pub winner: Side,
    pub loser: Side,
    /// Game files both change, with different content.
    pub files: usize,
    /// How the loser could win, when the app can do it: its archives renamed to load first.
    pub load_first: Option<Vec<RenameTo>>,
    /// What decides it, and how to change it by hand when the app cannot.
    pub how: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OverlapReport {
    pub archives: usize,
    pub overlaps: Vec<Overlap>,
    /// Resources two archives both carry with identical content (no effect in game).
    pub same_content: usize,
    pub unreadable: Vec<(String, String)>,
    pub modlist: Option<Vec<String>>,
    pub millis: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Owner { Install(i64), File(usize) }

/// Pairs of mods whose archives change the same game files, the winner first. `db` says who owns each archive.
pub fn overlaps(db: &Db, game: &Path) -> Result<OverlapReport> {
    let t0 = std::time::Instant::now();
    let list = load_order(game);
    // who owns which deployed path, with each install's name, source and renames (original target by new name)
    let mut owners: HashMap<String, i64> = HashMap::new();
    {
        let mut st = db.conn.prepare("SELECT target, install_id FROM deployed_files WHERE target LIKE '%.archive'")?;
        for r in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? { let (t, i) = r?; owners.insert(t, i); }
    }
    let mut info: HashMap<i64, (String, String, u32)> = HashMap::new();
    {
        let mut st = db.conn.prepare("SELECT id, name, source, mod_id FROM installs")?;
        for r in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, u32>(3)?)))? { let (i, n, s, m) = r?; info.insert(i, (n, s, m)); }
    }
    let mut rep = OverlapReport { archives: list.len(), modlist: modlist(game), ..Default::default() };
    let owner_of = |i: usize| list.get(i).and_then(|a| owners.get(&a.target.to_lowercase())).map(|id| Owner::Install(*id)).unwrap_or(Owner::File(i));

    // resource hash -> first archive that has it, and the content it has there
    let mut first: HashMap<u64, (usize, [u8; 20])> = HashMap::new();
    let mut pairs: BTreeMap<(Owner, Owner), (usize, BTreeSet<usize>, BTreeSet<usize>)> = BTreeMap::new();
    for (i, a) in list.iter().enumerate() {
        let entries = match entries_cached(&game.join(&a.target)) {
            Ok(e) => e,
            Err(e) => { rep.unreadable.push((a.target.clone(), e.to_string())); continue; }
        };
        let me = owner_of(i);
        for e in entries.iter() {
            match first.get(&e.hash) {
                None => { first.insert(e.hash, (i, e.sha1)); }
                Some((w, sha)) => {
                    let w = *w;
                    let them = owner_of(w);
                    if them == me { continue; }
                    if *sha == e.sha1 { rep.same_content += 1; continue; }
                    let p = pairs.entry((them, me.clone())).or_default();
                    p.0 += 1;
                    p.1.insert(w);
                    p.2.insert(i);
                }
            }
        }
    }

    let renames: HashMap<i64, BTreeMap<String, String>> = pairs.keys().filter_map(|(_, l)| if let Owner::Install(id) = l { Some(*id) } else { None })
        .map(|id| Ok((id, crate::rules::renames_for(db, id)?))).collect::<Result<_>>()?;
    let side = |o: &Owner, idx: &BTreeSet<usize>| -> Side {
        let archives: Vec<String> = idx.iter().map(|i| list[*i].target.clone()).collect();
        let redmod = idx.iter().any(|i| list[*i].redmod.is_some());
        match o {
            Owner::Install(id) => Side { install_id: Some(*id), mod_id: info.get(id).map(|x| x.2), name: info.get(id).map(|x| x.0.clone()).unwrap_or_else(|| format!("Install {id}")), archives, redmod },
            Owner::File(i) => Side { install_id: None, mod_id: None, name: list[*i].name.clone(), archives, redmod },
        }
    };
    for ((w, l), (n, wi, li)) in &pairs {
        let winner = side(w, wi);
        let loser = side(l, li);
        let source = loser.install_id.and_then(|id| info.get(&id)).map(|x| x.1.as_str());
        let (load_first, how) = if rep.modlist.is_some() {
            (None, "A modlist.txt in archive/pc/mod sets the order. Remove it (Health offers to), and file names decide again.".to_string())
        } else if loser.redmod {
            (None, format!("{} is a REDmod. REDmod archives load after every archive in archive/pc/mod{}, so it cannot win by a new name.",
                loser.name, if winner.redmod { ", and REDmods load by folder name" } else { "" }))
        } else if loser.install_id.is_none() {
            (None, format!("{} is not managed by Pockymod. To make it win, rename it in archive/pc/mod so its name sorts first, for example with ! in front.", loser.name))
        } else if source == Some("vortex") {
            (None, format!("{} is tracked from Vortex, and its files stay as Vortex named them. Install it in Pockymod to change its order here.", loser.name))
        } else {
            let before: Vec<String> = winner.archives.iter().map(|t| t.rsplit('/').next().unwrap_or(t).to_string()).collect();
            let map = loser.install_id.and_then(|id| renames.get(&id));
            let r: Option<Vec<RenameTo>> = loser.archives.iter().map(|t| {
                let file = t.rsplit('/').next().unwrap_or(t);
                // the name the mod's plan uses: a renamed archive is found by its new name
                let original = map.and_then(|m| m.iter().find(|(_, new)| new.eq_ignore_ascii_case(file)).map(|(orig, _)| orig.clone())).unwrap_or_else(|| t.clone());
                let base = original.rsplit('/').next().unwrap_or(&original).to_string();
                crate::rules::load_first_name(&base, &before).map(|name| RenameTo { target: original.clone(), name })
            }).collect();
            match r {
                Some(r) => (Some(r), "Archives load in name order and the first one wins. Players put ! in front of a name to load it earlier; Pockymod can do that to the link in the game and undo it.".into()),
                None => (None, "No name sorts before the winner's. Rename the winner instead.".into()),
            }
        };
        rep.overlaps.push(Overlap { winner, loser, files: *n, load_first, how });
    }
    rep.overlaps.sort_by(|a, b| b.files.cmp(&a.files).then_with(|| a.winner.name.cmp(&b.winner.name)));
    rep.millis = t0.elapsed().as_millis() as u64;
    Ok(rep)
}

/// Make an install's archives load first: deploy them under the given names from now on, and redeploy it now.
/// If the redeploy fails, the old names come back.
pub fn load_first(db: &Db, game: &Path, library: &Path, install_id: i64, renames: &[RenameTo], opts: &crate::deploy::DeployOptions) -> Result<crate::deploy::DeployOutcome> {
    let source = crate::install::source_of(db, install_id)?;
    if source == "vortex" { return Err(Error::Invalid("Mods tracked from Vortex keep Vortex's file names.".into())); }
    for r in renames {
        let ok = r.target.to_lowercase().starts_with("archive/pc/mod/") && r.name.to_lowercase().ends_with(".archive")
            && !r.name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']);
        if !ok { return Err(Error::Invalid(format!("{} cannot be renamed that way.", r.target))); }
    }
    let mod_id: u32 = db.conn.query_row("SELECT mod_id FROM installs WHERE id = ?1", params![install_id], |r| r.get(0))?;
    let before = crate::rules::renames_for(db, install_id)?;
    crate::install::undeploy_any(db, game, install_id)?;
    for r in renames { crate::rules::set_rename(db, install_id, &r.target, &r.name)?; }
    match crate::install::redeploy_any(db, game, library, install_id, opts) {
        Ok(d @ crate::deploy::DeployOutcome::Deployed { .. }) => Ok(d),
        other => {
            for r in renames {
                match before.get(&r.target.to_lowercase()) {
                    Some(old) => crate::rules::set_rename(db, install_id, &r.target, old)?,
                    None => crate::rules::clear_rename(db, mod_id, &r.target)?,
                }
            }
            let _ = crate::install::redeploy_any(db, game, library, install_id, opts);
            match other { Ok(d) => Ok(d), Err(e) => Err(e) }
        }
    }
}

/// Drop a rename: the archive goes back to its own name (redeployed now when its mod is in the game).
pub fn undo_rename(db: &Db, game: &Path, library: &Path, mod_id: u32, target: &str, opts: &crate::deploy::DeployOptions) -> Result<()> {
    let ids: Vec<i64> = {
        let mut st = db.conn.prepare("SELECT DISTINCT d.install_id FROM deployed_files d JOIN installs i ON i.id = d.install_id WHERE i.mod_id = ?1")?;
        let v = st.query_map(params![mod_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    for id in &ids { crate::install::undeploy_any(db, game, *id)?; }
    crate::rules::clear_rename(db, mod_id, target)?;
    for id in &ids { crate::install::redeploy_any(db, game, library, *id, opts)?; }
    Ok(())
}

/// Move a stray `modlist.txt` aside (to `modlist.txt.off` next to it), so archive names decide the order again.
pub fn disable_modlist(game: &Path) -> Result<bool> {
    let p = game.join(MODLIST);
    if !p.is_file() { return Ok(false); }
    let off = p.with_extension("txt.off");
    let _ = fs::remove_file(&off);
    fs::rename(&p, &off)?;
    Ok(true)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// An archive the way WolvenKit writes one: header, padding to 0xAC, data, the index on a page boundary.
    pub(crate) fn build(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = vec![0u8; 0xAC];
        let mut entries = Vec::new();
        let mut segments = Vec::new();
        for (path, body) in files {
            let offset = out.len() as u64;
            out.extend_from_slice(body);
            let mut sha = [0u8; 20];
            for (i, b) in body.iter().enumerate() { sha[i % 20] = sha[i % 20].wrapping_add(*b).rotate_left(3); }
            entries.push((fnv1a64(path), segments.len() as u32, sha));
            segments.push((offset, body.len() as u32));
        }
        while out.len() % 4096 != 0 { out.push(0); }
        let index_pos = out.len() as u64;
        let mut table = Vec::new();
        table.extend((entries.len() as u32).to_le_bytes());
        table.extend((segments.len() as u32).to_le_bytes());
        table.extend(0u32.to_le_bytes());
        for (hash, seg, sha) in &entries {
            table.extend(hash.to_le_bytes());
            table.extend(133_000_000_000_000_000i64.to_le_bytes());
            table.extend(0u32.to_le_bytes());
            table.extend(seg.to_le_bytes());
            table.extend((seg + 1).to_le_bytes());
            table.extend(0u32.to_le_bytes());
            table.extend(0u32.to_le_bytes());
            table.extend(sha);
        }
        for (off, size) in &segments { table.extend(off.to_le_bytes()); table.extend(size.to_le_bytes()); table.extend(size.to_le_bytes()); }
        out.extend(8u32.to_le_bytes());
        out.extend((table.len() as u32 + 8).to_le_bytes());
        out.extend(0u64.to_le_bytes());
        out.extend(&table);
        let index_size = out.len() as u64 - index_pos;
        while out.len() % 4096 != 0 { out.push(0); }
        let size = out.len() as u64;
        let mut h = Vec::new();
        h.extend(MAGIC.to_le_bytes());
        h.extend(12u32.to_le_bytes());
        h.extend(index_pos.to_le_bytes());
        h.extend((index_size as u32).to_le_bytes());
        h.extend(0u64.to_le_bytes());
        h.extend(0u32.to_le_bytes());
        h.extend(size.to_le_bytes());
        out[..HEADER].copy_from_slice(&h);
        out
    }

    #[test]
    fn reads_the_file_table_of_an_archive() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.archive");
        fs::write(&p, build(&[("base\\characters\\common\\hair\\h1.mesh", b"mesh one"), ("base\\gameplay\\gui\\x.inkwidget", b"widget")])).unwrap();
        let e = read_entries(&p).unwrap();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].hash, fnv1a64("base\\characters\\common\\hair\\h1.mesh"));
        assert_ne!(e[0].sha1, e[1].sha1);
        assert_eq!(entries_cached(&p).unwrap().len(), 2);
        // not an archive, or cut short
        fs::write(&p, b"PK\x03\x04 not an archive at all, just long enough to hold a header").unwrap();
        assert!(read_entries(&p).is_err());
        let mut short = build(&[("x", b"y")]);
        short.truncate(4100);
        fs::write(&p, short).unwrap();
        assert!(read_entries(&p).is_err());
    }

    #[test]
    fn fnv_matches_the_reference() {
        assert_eq!(fnv1a64(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64("a"), 0xaf63_dc4c_8601_ec8c);
    }

    fn game_with(archives: &[(&str, &[(&str, &[u8])])]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let g = dir.path().join("game");
        for (rel, files) in archives {
            let p = g.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, build(files)).unwrap();
        }
        (dir, g)
    }

    #[test]
    fn load_order_by_name_then_redmods_and_modlist() {
        let (_d, g) = game_with(&[("archive/pc/mod/b.archive", &[]), ("archive/pc/mod/!a.archive", &[]), ("archive/pc/mod/B2.archive", &[]),
            ("mods/zMod/archives/z.archive", &[]), ("mods/AMod/archives/y.archive", &[])]);
        let names: Vec<String> = load_order(&g).into_iter().map(|a| a.target).collect();
        assert_eq!(names, ["archive/pc/mod/!a.archive", "archive/pc/mod/B2.archive", "archive/pc/mod/b.archive", "mods/AMod/archives/y.archive", "mods/zMod/archives/z.archive"]);
        fs::write(g.join(MODLIST), "b.archive\r\n\r\n").unwrap();
        let names: Vec<String> = load_order(&g).into_iter().map(|a| a.name).collect();
        assert_eq!(&names[..3], ["b.archive", "!a.archive", "B2.archive"]);
        assert!(disable_modlist(&g).unwrap());
        assert!(modlist(&g).is_none() && g.join("archive/pc/mod/modlist.txt.off").exists());
    }

    #[test]
    fn overlapping_archives_pair_up_with_the_winner_first() {
        let hair: &[u8] = b"hair by A";
        let (_d, g) = game_with(&[
            ("archive/pc/mod/A_hair.archive", &[("base\\h.mesh", hair), ("base\\only_a.mesh", b"a")]),
            ("archive/pc/mod/B_hair.archive", &[("base\\h.mesh", b"hair by B"), ("base\\shared_same.xbm", b"same")]),
            ("archive/pc/mod/C.archive", &[("base\\shared_same.xbm", b"same")]),
            ("mods/RedHair/archives/r.archive", &[("base\\h.mesh", b"hair by R")]),
        ]);
        let db = Db::memory().unwrap();
        for (id, name, target) in [(1, "Hair A", "archive/pc/mod/a_hair.archive"), (2, "Hair B", "archive/pc/mod/b_hair.archive"), (3, "Red hair", "mods/redhair/archives/r.archive")] {
            db.conn.execute("INSERT INTO installs (id, mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at) VALUES (?1, ?1, ?1, ?2, '1', 'archive', '[]', '', 0)", params![id, name]).unwrap();
            db.conn.execute("INSERT INTO deployed_files (target, install_id, source, method, size, deployed_at, target_path) VALUES (?1, ?2, '', 'hardlink', 0, 0, ?1)", params![target, id]).unwrap();
        }
        let r = overlaps(&db, &g).unwrap();
        assert_eq!(r.archives, 4);
        assert_eq!(r.same_content, 1, "B and C carry the same file: no effect");
        assert_eq!(r.overlaps.len(), 2, "{:?}", r.overlaps);
        let ab = r.overlaps.iter().find(|o| o.loser.name == "Hair B").unwrap();
        assert_eq!((ab.winner.name.as_str(), ab.files), ("Hair A", 1));
        assert_eq!(ab.load_first.as_ref().unwrap()[0], RenameTo { target: "archive/pc/mod/B_hair.archive".into(), name: "!B_hair.archive".into() });
        let ar = r.overlaps.iter().find(|o| o.loser.name == "Red hair").unwrap();
        assert!(ar.loser.redmod && ar.load_first.is_none(), "a REDmod cannot win by a name");
        // a second look is served from the cache
        assert_eq!(overlaps(&db, &g).unwrap().overlaps.len(), 2);
    }
}
