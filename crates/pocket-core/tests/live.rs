//! Live checks against the real Nexus API, anonymous. Not run by default: `cargo test -p pocket-core -- --ignored`.

use pocket_core::browse::{self, LocalFilter};
use pocket_core::cyberpunk::frameworks::Framework;
use pocket_core::db::{Db, Flag};
use pocket_core::nexus::graphql::{BrowseQuery, SortKey};
use pocket_core::nexus::NexusClient;
use std::sync::Mutex;

#[tokio::test]
#[ignore]
async fn search_detail_and_cache() {
    let client = NexusClient::new(None).unwrap();
    let db = Mutex::new(Db::memory().unwrap());
    let q = BrowseQuery { text: Some("vehicle".into()), sort: SortKey::Relevance, frameworks: vec![Framework::Cet], count: 10, ..Default::default() };
    let r = browse::search(&client, &db, &q, &LocalFilter::default()).await.unwrap();
    assert!(r.total > 0 && !r.nodes.is_empty(), "results");
    assert!(!r.facets.categories.is_empty(), "facet counts in the same response");
    assert!(r.derived.iter().any(|(k, n)| k == "fw_cet" && *n == r.total), "derived count for the applied framework equals the total");
    assert!(!r.from_cache);
    let again = browse::search(&client, &db, &q, &LocalFilter::default()).await.unwrap();
    assert!(again.from_cache, "second search is served from the cache");

    // hide the first result: it drops out unless hidden mods are shown
    let first = r.nodes[0].card.mod_id;
    db.lock().unwrap().set_flag(first, Flag::Hidden, true).unwrap();
    let hidden = browse::search(&client, &db, &q, &LocalFilter::default()).await.unwrap();
    assert!(hidden.nodes.iter().all(|n| n.card.mod_id != first) && hidden.filtered_on_page == 1);

    // ArchiveXL: requirement RED4ext, its own type, a file list from the index (older file, kept under file 0)
    let d = browse::detail(&client, &db, 4198, None).await.unwrap();
    assert_eq!(d.classification.is_framework, Some(Framework::ArchiveXl));
    assert!(d.classification.frameworks.iter().any(|f| f.framework == Framework::Red4ext));
    assert!(d.file_list.as_ref().is_some_and(|l| l.iter().any(|p| p.ends_with("ArchiveXL.dll"))), "file list: {:?}", d.file_list);
    println!("search total {} · first {} · ArchiveXL files {:?}", r.total, r.nodes[0].card.name, d.file_list.map(|l| l.len()));
}

#[tokio::test]
#[ignore]
async fn collections_browse_revision_and_check() {
    use pocket_core::collections::{self, CollectionQuery};
    let client = NexusClient::new(None).unwrap();
    let db = Mutex::new(Db::memory().unwrap());
    let page = collections::browse(&client, &db, &CollectionQuery::default()).await.unwrap();
    assert!(page.total_count > 0 && !page.nodes.is_empty());
    assert!(page.facets_data.as_ref().and_then(|f| f.get("gameVersion")).is_some(), "game version facet");
    let slug = page.nodes[0].slug.clone();
    let rev = collections::revision(&client, &slug, None).await.unwrap();
    assert!(!rev.mod_files.is_empty());
    let mut check = collections::check_revision(&db.lock().unwrap(), &rev, Some("2.5".into()), false).unwrap();
    assert_eq!(check.installed, 0);
    collections::resolve_files(&client, &db, &mut check, 3, false).await.unwrap();
    println!("{}: {} mods, {} missing, {} unavailable, {} clicks, game ok {:?}", check.name, check.entries.len(), check.missing, check.unavailable, check.needs_click, check.game_version_ok);
}

#[tokio::test]
#[ignore]
async fn author_tools() {
    use pocket_core::creator::mods;
    let client = NexusClient::new(None).unwrap();
    let db = Mutex::new(Db::memory().unwrap());
    // the uploader of ArchiveXL, by name and by id
    let id = mods::resolve_member(&client, "psiberx").await.unwrap();
    assert_eq!(id, 108159138);
    let my = mods::my_mods(&client, &db, id, &[], false, false).await.unwrap();
    assert!(my.mods.iter().any(|m| m.mod_id == 4198), "ArchiveXL is theirs");
    assert!(my.mods.iter().all(|m| m.uploader.as_ref().is_some_and(|u| u.member_id == id)));
    assert!(my.mods.iter().any(|m| m.unique_downloads.is_some()), "unique downloads from v3");
    assert!(my.mods.iter().all(|m| m.files.iter().all(|f| f.group_id.is_some())));
    let m = mods::manage(&client, &db, 4198).await.unwrap();
    assert!(m.required_by_total > 100 && !m.changelog.is_empty());
    println!("{}: {} mods, {} downloads; ArchiveXL required by {}", my.author.name, my.totals.mods, my.totals.downloads, m.required_by_total);
}
