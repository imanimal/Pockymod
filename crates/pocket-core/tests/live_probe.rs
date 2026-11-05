//! Live check of the Nexus API (milestone 2). Not run by default: it needs a real personal API key.
//!
//!   set NEXUS_API_KEY=<your key>          (PowerShell: $env:NEXUS_API_KEY="<your key>")
//!   cargo test -p pocket-core --test live_probe -- --ignored --nocapture
//!
//! It validates the key, reads the rate-limit headers, introspects the v2 `Mod`, `ModsFilter` and `ModsSort`
//! types, runs one small `mods()` query for cyberpunk2077, and writes everything to target/nexus-probe.json.
//! Compare `requestedButMissing` and `sampleResponse` with the unconfirmed list in docs/build-prompt.md §4.

use pocket_core::nexus::Client;

#[tokio::test]
#[ignore = "needs NEXUS_API_KEY and network"]
async fn probe_live_api() {
    let key = std::env::var("NEXUS_API_KEY").expect("set NEXUS_API_KEY to run the live probe");
    let client = Client::new(&key).unwrap();
    let account = client.validate().await.expect("users/validate");
    println!("account: {} (premium: {})", account.name, account.is_premium);
    let report = client.probe().await.expect("probe");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nexus-probe.json");
    std::fs::write(&out, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    println!("written to {}", out.display());
    let rate = client.rate();
    assert!(rate.daily_remaining.is_some() || rate.hourly_remaining.is_some(), "Nexus sent no x-rl-* headers");
}
