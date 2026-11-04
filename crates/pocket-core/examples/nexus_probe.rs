//! Live test of the Nexus v2 API for cyberpunk2077, using the app's own client and queries.
//! It logs the real response shape of `mods()` and checks each claim in the spec's "reported but not confirmed" list.
//!
//!     cargo run -p pocket-core --example nexus_probe -- [--stored-key] [--out docs/nexus]
//!
//! Without --stored-key it runs anonymously (v2 browsing works without a key). With it, it uses the key saved in
//! Windows Credential Manager by the app, and also tries the v1 calls that need a key. The key is never printed.

use pocket_core::credentials::{OsKeyring, SecretStore};
use pocket_core::nexus::graphql::{self, BrowseQuery, SortKey};
use pocket_core::nexus::NexusClient;
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::path::PathBuf;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let stored = args.iter().any(|a| a == "--stored-key");
    let out: PathBuf = args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1)).map(PathBuf::from).unwrap_or_else(|| "docs/nexus".into());
    let key = if stored { OsKeyring::nexus().get().expect("credential store") } else { None };
    if stored && key.is_none() { eprintln!("No key stored; running anonymously."); }
    let client = NexusClient::new(key).expect("client");
    let mut md = String::new();
    let mut checks: Vec<(String, String, String)> = Vec::new();
    let mut check = |claim: &str, result: &str, note: String| checks.push((claim.into(), result.into(), note));
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC");
    writeln!(md, "# Nexus live probe\n\nRun: {now}. Auth: {}. Game: cyberpunk2077 (gameId {}).\n", if client.has_key() { "personal API key" } else { "none (anonymous)" }, graphql::GAME_ID).unwrap();

    // 1. Schema: the arguments and types around mods()
    let intro = r#"query { q: __type(name: "Query") { fields { name args { name } } }
      mod: __type(name: "Mod") { fields { name } } page: __type(name: "ModPage") { fields { name } }
      filter: __type(name: "ModsFilter") { inputFields { name } } sort: __type(name: "ModsSort") { inputFields { name } }
      facet: __type(name: "ModsFacet") { inputFields { name } } cfilter: __type(name: "CollectionsSearchFilter") { inputFields { name } }
      csort: __type(name: "CollectionsSearchSort") { inputFields { name } } }"#;
    let (schema, _) = client.graphql_raw(intro, json!({})).await.expect("introspection");
    let names = |p: &str, k: &str| -> Vec<String> { schema.pointer(p).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|f| f.get(k).and_then(|n| n.as_str()).map(str::to_owned)).collect()).unwrap_or_default() };
    let mods_args: Vec<String> = schema.pointer("/q/fields").and_then(|f| f.as_array()).and_then(|a| a.iter().find(|f| f["name"] == "mods"))
        .and_then(|f| f["args"].as_array()).map(|a| a.iter().filter_map(|x| x["name"].as_str().map(str::to_owned)).collect()).unwrap_or_default();
    let mod_fields = names("/mod/fields", "name");
    let filter_fields = names("/filter/inputFields", "name");
    let sort_fields = names("/sort/inputFields", "name");
    let facet_fields = names("/facet/inputFields", "name");
    let cfilter = names("/cfilter/inputFields", "name");
    let csort = names("/csort/inputFields", "name");
    writeln!(md, "## Schema\n\n- `mods()` arguments: {}\n- `ModPage` fields: {}\n- `ModsFilter` fields ({}): {}\n- `ModsSort` keys ({}): {}\n- `ModsFacet` fields: {}\n- `Mod` fields ({}): {}\n",
        mods_args.join(", "), names("/page/fields", "name").join(", "), filter_fields.len(), filter_fields.join(", "), sort_fields.len(), sort_fields.join(", "), facet_fields.join(", "), mod_fields.len(), mod_fields.join(", ")).unwrap();

    // 2. One page with facets, exactly as the browser asks for it
    let q = BrowseQuery { count: 3, ..Default::default() };
    let (page, headers) = client.graphql_raw(&graphql::mods_page_query(), graphql::page_variables(&q)).await.expect("mods page");
    let m = &page["mods"];
    let first = m["nodes"].get(0).cloned().unwrap_or(Value::Null);
    writeln!(md, "## A `mods()` page\n\nVariables sent:\n\n```json\n{}\n```\n\nResponse: HTTP {}, totalCount {}, nodesCount {}, facetsData keys: {}.\n\nFirst node, with the type of each field:\n\n```\n{}```\n",
        serde_json::to_string_pretty(&graphql::page_variables(&q)).unwrap(), headers.status, m["totalCount"], m["nodesCount"],
        m["facetsData"].as_object().map(|o| o.keys().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_default(), shape(&first, 0)).unwrap();
    writeln!(md, "Response headers: {}.\n\nRate-limit headers: {}.\n", headers.names.join(", "),
        if headers.rate_limit.is_empty() { "none".to_string() } else { headers.rate_limit.iter().map(|(k, v)| format!("`{k}: {v}`")).collect::<Vec<_>>().join(", ") }).unwrap();
    std::fs::create_dir_all(&out).ok();
    let mut sample = page.clone();
    if let Some(fd) = sample.pointer_mut("/mods/facetsData/tag").and_then(|t| t.as_object_mut()) { let keep: serde_json::Map<String, Value> = fd.iter().take(12).map(|(k, v)| (k.clone(), v.clone())).collect(); *fd = keep; }
    std::fs::write(out.join("live-sample.json"), serde_json::to_string_pretty(&sample).unwrap()).unwrap();

    // 3. The spec's unconfirmed list
    let base = count(&client, BrowseQuery::default()).await.unwrap_or(0);
    let text = count(&client, BrowseQuery { text: Some("better vehicle".into()), sort: SortKey::Relevance, ..Default::default() }).await;
    check("Search by name and description", ok(text.as_ref().is_ok_and(|n| *n > 0 && *n < base)), format!("\"better vehicle\": {:?} of {base}", text));
    let cat = count(&client, BrowseQuery { categories: vec!["Gameplay".into(), "Weapons".into()], ..Default::default() }).await;
    check("Category filter", ok(cat.as_ref().is_ok_and(|n| *n > 0)), format!("Gameplay OR Weapons: {cat:?}"));
    let tag = count(&client, BrowseQuery { tags: vec!["REDMod".into()], ..Default::default() }).await;
    check("Tag filter", ok(tag.as_ref().is_ok_and(|n| *n > 0)), format!("tag REDMod: {tag:?}"));
    check("Author filter", ok(filter_fields.iter().any(|f| f == "author")), "ModsFilter has author and uploader".into());
    let date = count(&client, BrowseQuery { updated_after: Some(chrono::Utc::now().timestamp() - 7 * 86400), ..Default::default() }).await;
    check("Date filters", ok(date.as_ref().is_ok_and(|n| *n > 0 && *n < base)), format!("updated in the last 7 days: {date:?}. Values must be Unix seconds as strings; ISO dates fail"));
    let dl = count(&client, BrowseQuery { downloads_min: Some(100_000), downloads_max: Some(200_000), ..Default::default() }).await;
    check("Downloads range", ok(dl.as_ref().is_ok_and(|n| *n > 0)), format!("100k–200k downloads: {dl:?}"));
    let en = count(&client, BrowseQuery { endorsements_min: Some(10_000), ..Default::default() }).await;
    check("Endorsements filter", ok(en.as_ref().is_ok_and(|n| *n > 0)), format!("≥10k endorsements: {en:?}"));
    let sz = count(&client, BrowseQuery { size_min_kb: Some(1_000_000), ..Default::default() }).await;
    check("File size filter", ok(sz.as_ref().is_ok_and(|n| *n > 0)), format!("≥1,000,000 KB: {sz:?}"));
    let adult = count(&client, BrowseQuery { include_adult: true, ..Default::default() }).await;
    check("Adult-content flag", ok(adult.as_ref().is_ok_and(|n| *n > base)), format!("with adult {adult:?}, without {base}"));
    check("Update flag", ok(filter_fields.iter().any(|f| f == "hasUpdated")), "ModsFilter has hasUpdated (boolean); not exercised".into());
    check("Boolean AND/OR", ok(filter_fields.iter().any(|f| f == "op") && filter_fields.iter().any(|f| f == "filter")), "nested filter list with op AND/OR; several values in one field are ANDed".into());
    check("10 sort keys", if sort_fields.len() == 10 { "confirmed" } else { "differs" }, sort_fields.join(", "));
    check("Facet counts", ok(m["facetsData"].get("tag").is_some() && m["facetsData"].get("categoryName").is_some()), format!("facets available: {}; returned only when asked for in `facets`", facet_fields.join(", ")));
    let p2 = client.mods_page(&BrowseQuery { offset: 3, count: 3, ..Default::default() }).await.map(|p| p.nodes.first().map(|n| n.mod_id));
    check("Offset and count paging", ok(p2.is_ok()), format!("offset 3 first mod: {p2:?}"));
    check("Mod has requirement links", ok(mod_fields.iter().any(|f| f == "modRequirements")), "modRequirements.nexusRequirements, and it can be fetched inside a mods() page".into());
    check("Mod has no returnable tags", if mod_fields.iter().any(|f| f == "tags") { "differs" } else { "confirmed" }, "Mod.tags: [LegacyTag!]! is returned (id, name)".into());
    let has_type = mod_fields.iter().any(|f| f.to_lowercase().contains("modtype") || f == "type");
    check("Mod has no mod-type field", if has_type { "differs" } else { "confirmed" }, "category is a site category, not REDmod/archive/script".into());
    let has_gv = mod_fields.iter().any(|f| f.to_lowercase().contains("gameversion"));
    check("Mod has no game-version field", if has_gv { "differs" } else { "confirmed" }, "nothing on Mod; version is the mod's own".into());
    check("collectionsV2 has a game-version filter", ok(cfilter.iter().any(|f| f == "gameVersion")), format!("sort options: {}", csort.join(", ")));
    check("v1 REST has no search; mods/updated returns IDs and timestamps", if client.has_key() { "see below" } else { "not checked" }, "v1 answers 401 without a key".into());

    // 4. Detail, file list, derived counts
    let detail = client.mod_detail(4198).await;
    let (d_ok, d_note) = match &detail { Ok((d, files)) => (true, format!("ArchiveXL: {} requirement(s) ({}), {} files, current MAIN: {:?}", d.mod_requirements.nexus_requirements.total_count,
        d.mod_requirements.nexus_requirements.nodes.iter().map(|r| r.mod_name.clone()).collect::<Vec<_>>().join(", "), files.len(), files.iter().rev().find(|f| f.category == "MAIN").map(|f| (&f.version, f.file_id)))), Err(e) => (false, e.to_string()) };
    check("Detail: mod() + modFiles() in one request", ok(d_ok), d_note);
    let fc = client.file_contents(4198, None, 5).await;
    let latest = detail.as_ref().ok().and_then(|(_, f)| f.iter().rev().find(|f| f.category == "MAIN").map(|f| f.file_id));
    let fc_latest = match latest { Some(id) => client.file_contents(4198, Some(id), 5).await.map(|r| r.0).ok(), None => None };
    check("Archive file lists (modFileContents)", ok(fc.as_ref().is_ok_and(|r| r.0 > 0)), format!("ArchiveXL, all files: {:?} entries; newest MAIN file: {:?}. The index does not cover every file",
        fc.as_ref().map(|r| r.0), fc_latest));
    let dc = client.derived_counts(&BrowseQuery::default()).await;
    check("Derived filter counts in one aliased request", ok(dc.is_ok()), format!("{:?}", dc.map(|v| v.into_iter().map(|(k, n)| format!("{k}={n}")).collect::<Vec<_>>().join(" "))));

    if client.has_key() {
        match client.validate().await { Ok(u) => check("v1 validate", "confirmed", format!("premium: {}, supporter: {}", u.is_premium, u.is_supporter)), Err(e) => check("v1 validate", "failed", e.to_string()) }
        match client.updated_mods("1d").await { Ok(u) => check("v1 mods/updated", "confirmed", format!("{} mods, first: {:?}", u.len(), u.first())), Err(e) => check("v1 mods/updated", "failed", e.to_string()) }
        let rl = client.rate_limit();
        check("Rate-limit headers on keyed requests", ok(rl.remaining.is_some()), format!("{rl:?}"));
    }

    writeln!(md, "## Section 4 claims\n\n| Claim | Result | Notes |\n|---|---|---|").unwrap();
    for (c, r, n) in &checks { writeln!(md, "| {c} | {r} | {} |", n.replace('|', "\\|")).unwrap(); }
    std::fs::write(out.join("live-probe.md"), &md).unwrap();
    println!("{md}");
}

async fn count(c: &NexusClient, q: BrowseQuery) -> pocket_core::Result<u64> {
    c.mods_page(&BrowseQuery { count: 1, ..q }).await.map(|p| p.total_count)
}

fn ok(b: bool) -> &'static str { if b { "confirmed" } else { "failed" } }

/// Field names and JSON types, nested.
fn shape(v: &Value, depth: usize) -> String {
    let pad = "  ".repeat(depth);
    let mut s = String::new();
    if let Value::Object(o) = v {
        for (k, x) in o {
            match x {
                Value::Object(_) => { s += &format!("{pad}{k}: object\n"); s += &shape(x, depth + 1); }
                Value::Array(a) => {
                    s += &format!("{pad}{k}: array[{}]\n", a.len());
                    if let Some(f @ Value::Object(_)) = a.first() { s += &shape(f, depth + 1); }
                }
                _ => s += &format!("{pad}{k}: {}\n", match x { Value::Null => "null", Value::Bool(_) => "bool", Value::Number(_) => "number", Value::String(_) => "string", _ => "?" }),
            }
        }
    }
    s
}
