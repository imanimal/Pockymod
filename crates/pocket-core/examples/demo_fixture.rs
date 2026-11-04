//! Writes the demo data the UI uses outside Tauri (src/lib/demo/*.json), from live anonymous Nexus responses passed
//! through the same browse code the app runs.
//!
//!     cargo run -p pocket-core --example demo_fixture -- src/lib/demo

use pocket_core::browse::{self, LocalFilter};
use pocket_core::db::Db;
use pocket_core::nexus::graphql::BrowseQuery;
use pocket_core::nexus::NexusClient;
use std::sync::Mutex;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "src/lib/demo".into()));
    std::fs::create_dir_all(&out).unwrap();
    let client = NexusClient::new(None).unwrap();
    let db = Mutex::new(Db::memory().unwrap());
    let r = browse::search(&client, &db, &BrowseQuery { count: 40, ..Default::default() }, &LocalFilter::default()).await.unwrap();
    std::fs::write(out.join("browse.json"), serde_json::to_string_pretty(&r).unwrap()).unwrap();
    let mut details = Vec::new();
    for n in r.nodes.iter().take(8) {
        let mut d = browse::detail(&client, &db, n.card.mod_id, None).await.unwrap();
        // keep the fixture small: the description's first part and the newest files
        d.detail.description = d.detail.description.chars().take(6000).collect();
        d.files.sort_by_key(|f| std::cmp::Reverse(f.date));
        d.files.truncate(8);
        details.push(d);
    }
    std::fs::write(out.join("details.json"), serde_json::to_string_pretty(&details).unwrap()).unwrap();
    let mut cols = pocket_core::collections::browse(&client, &db, &Default::default()).await.unwrap();
    cols.nodes.truncate(9);
    std::fs::write(out.join("collections.json"), serde_json::to_string_pretty(&cols).unwrap()).unwrap();
    let rev = pocket_core::collections::revision(&client, &cols.nodes[0].slug, None).await.unwrap();
    let mut check = pocket_core::collections::check_revision(&db.lock().unwrap(), &rev, Some("2.5".into()), false).unwrap();
    check.entries.truncate(60);
    let imgs = browse::media(&client, &db, "image", "rating", 0).await.unwrap();
    let vids = browse::media(&client, &db, "video", "views", 0).await.unwrap();
    std::fs::write(out.join("media.json"), serde_json::to_string_pretty(&serde_json::json!({ "image": imgs, "video": vids })).unwrap()).unwrap();
    std::fs::write(out.join("collection-check.json"), serde_json::to_string_pretty(&check).unwrap()).unwrap();
    println!("{} cards, {} details", r.nodes.len(), details.len());
}
