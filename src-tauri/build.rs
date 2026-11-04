//! Tauri's build step, and the marketplace bundled into the app: every JSON file in `../marketplace` is embedded, so
//! the Add-ons screen has a marketplace before one is published online and when GitHub cannot be reached
//! (src/commands/addons.rs uses it only when there is no online or saved copy).
use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../marketplace");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    for sub in ["", "themes", "extensions"] {
        let dir = if sub.is_empty() { root.clone() } else { root.join(sub) };
        println!("cargo:rerun-if-changed={}", dir.display());
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "json") && p.is_file() {
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                let rel = if sub.is_empty() { name } else { format!("{sub}/{name}") };
                println!("cargo:rerun-if-changed={}", p.display());
                files.push((rel, p.canonicalize().unwrap_or(p)));
            }
        }
    }
    files.sort();
    let mut out = String::from("/// The marketplace as it was when this app was built: (path in the folder, file text).\npub static BUNDLED: &[(&str, &str)] = &[\n");
    for (rel, p) in &files { let _ = writeln!(out, "    ({rel:?}, include_str!({:?})),", p.to_string_lossy()); }
    out.push_str("];\n");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("market_bundle.rs"), out).expect("write the marketplace bundle");
    tauri_build::build()
}
