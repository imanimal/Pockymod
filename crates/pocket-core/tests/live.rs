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
