//! Records the fixtures for the author tools from live, anonymous Nexus answers:
//! - `crates/pocket-core/tests/fixtures/creator/*.json`: raw answers the unit tests parse;
//! - `src/lib/demo/my-mods.json` and `my-mod-files.json`: the parsed results the demo backend gives the UI.
//!
//!     cargo run -p pocket-core --example creator_fixture -- <member id> <manage mod id>
//!
//! Without arguments it records member 108159138 (the uploader of ArchiveXL) and mod 18839.

use pocket_core::creator::mods;
use pocket_core::db::Db;
use pocket_core::nexus::graphql::GAME_ID;
use pocket_core::nexus::NexusClient;
use serde_json::{json, Map, Value};
use std::path::Path;
use std::sync::Mutex;

fn write(path: &Path, v: &impl serde::Serialize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_string_pretty(v).unwrap() + "\n").unwrap();
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let member: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(108159138);
    let manage_id: u32 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(18839);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures = root.join("crates/pocket-core/tests/fixtures/creator");
    let demo = root.join("src/lib/demo");
    let client = NexusClient::new(None).unwrap();
    let db = Mutex::new(Db::memory().unwrap());

    let pages = mods::fetch_author_pages(&client, member).await.unwrap();
    write(&fixtures.join("author-page.json"), &pages[0]);
    let ids: Vec<u32> = pages[0]["mods"]["nodes"].as_array().unwrap().iter().filter_map(|m| m["modId"].as_u64().map(|n| n as u32)).collect();
    let mut all = Map::new();
    for d in mods::fetch_files(&client, &ids).await.unwrap() { all.extend(d.as_object().cloned().unwrap_or_default()); }
    write(&fixtures.join("files.json"), &mods::trim_files_answer(&Value::Object(all), 6));
    let (manage, _) = client.graphql_raw(mods::MANAGE_QUERY, json!({ "modId": manage_id.to_string(), "gameId": GAME_ID.to_string() })).await.unwrap();
    write(&fixtures.join("manage.json"), &manage);
    let mut v3 = mods::fetch_v3_mod(&client, manage_id).await.unwrap();
    // the description is not used; keep the fixture short
    if let Some(d) = v3.pointer_mut("/data/description") { *d = json!(""); }
    write(&fixtures.join("v3-mod.json"), &v3);

    let my = mods::my_mods(&client, &db, member, &[], false, true).await.unwrap();
    write(&demo.join("my-mods.json"), &my);
    let mut views = Vec::new();
    for m in &my.mods {
        let mut v = mods::manage(&client, &db, m.mod_id).await.unwrap();
        // keep the demo data small: the newest files and the first mods that require it
        v.files.truncate(16);
        v.changelog.truncate(12);
        v.required_by.truncate(20);
        views.push(v);
    }
    write(&demo.join("my-mod-files.json"), &views);
    println!("{} mods for {} · {} manage views", my.mods.len(), my.author.name, views.len());
}
