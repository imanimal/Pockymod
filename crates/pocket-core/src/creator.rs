//! For mod authors: check a mod before it goes up, pack it the way managers expect, and publish a new version.
//!
//! Publishing uses Nexus's Upload API (open beta since 2026-03, "focused on updating your mods"). The calls follow
//! Nexus's own GitHub Action (github.com/Nexus-Mods/upload-action, read 2026-11-04), base https://api.nexusmods.com/v3:
//!   POST /uploads/multipart { filename, size_bytes } → { data: { id, part_presigned_urls[], part_size_bytes, complete_presigned_url } }
//!   PUT each part to its URL (keep the ETag) → POST the CompleteMultipartUpload XML to complete_presigned_url
//!   POST /uploads/{id}/finalise → poll GET /uploads/{id} until data.state == "available"
//!   POST /mod-files/{file_id}/versions { upload_id, name, version, description?, file_category (main|optional|miscellaneous),
//!        archive_existing_file, update_mod_version, ... } → { data: { version: { id } } }
//!   POST /mods/{mod_id}/changelogs { version, changelog }
//! UNVERIFIED here (needs an author's key): every call. A brand-new mod page still needs the website's upload form.

use crate::archive;
use crate::cyberpunk::layout::{plan_install, FileKind, InstallPlan};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const UPLOAD_BASE: &str = "https://api.nexusmods.com/v3";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub plan: InstallPlan,
    /// Problems that make installs fail or land in the wrong place, in plain words.
    pub problems: Vec<String>,
    /// Things that work but could be better.
    pub tips: Vec<String>,
    pub files: usize,
}

fn listing(path: &Path) -> Result<Vec<String>> {
    if path.is_dir() {
        let mut v: Vec<String> = walkdir::WalkDir::new(path).into_iter().flatten().filter(|e| e.file_type().is_file())
            .filter_map(|e| e.path().strip_prefix(path).ok().map(|p| p.to_string_lossy().replace('\\', "/"))).collect();
        v.sort();
        Ok(v)
    } else {
        archive::list(path)
    }
}

/// What players' managers will do with this folder or archive, and what to fix first.
pub fn check(path: &Path, name: &str) -> Result<Check> {
    let entries = listing(path)?;
    let plan = plan_install(&entries, name);
    let mut problems = Vec::new();
    let mut tips = Vec::new();
    if crate::fomod::find_config(&entries).is_some() { tips.push("It has a FOMOD installer: players choose options. Check every option installs something.".into()); }
    if !plan.unknown.is_empty() { problems.push(format!("{} files have no place in the game folder (for example {}). Managers skip them or stop.", plan.unknown.len(), plan.unknown[0])); }
    if plan.files.is_empty() { problems.push("Nothing here installs into the game folder.".into()); }
    let loose_archives = entries.iter().filter(|e| e.to_lowercase().ends_with(".archive") && !e.to_lowercase().contains("archive/pc/mod/") && !e.to_lowercase().contains("/archives/")).count();
    if loose_archives > 0 { tips.push(format!("{loose_archives} .archive files are not inside archive/pc/mod. Pocket Mods and Vortex place them, but a manual install by a player may not.")); }
    let top: std::collections::BTreeSet<&str> = entries.iter().filter_map(|e| e.split('/').next()).collect();
    if top.len() == 1 && !["archive", "bin", "engine", "mods", "r6", "red4ext"].contains(&top.iter().next().map(|s| s.to_lowercase()).unwrap_or_default().as_str()) && entries.iter().all(|e| e.contains('/')) {
        tips.push("Everything sits in one extra top folder. It works here, but packing from inside it (archive, r6 … at the top) suits every manager.".into());
    }
    if plan.files.iter().any(|p| p.kind == FileKind::Redmod) && !entries.iter().any(|e| e.to_lowercase().ends_with("info.json")) {
        problems.push("REDmod files without an info.json: the game's REDmod tool will not see the mod.".into());
    }
    if !plan.warnings.is_empty() { problems.extend(plan.warnings.iter().filter(|w| w.contains("same path")).cloned()); }
    if entries.iter().any(|e| e.to_lowercase().ends_with(".exe") || e.to_lowercase().ends_with(".msi")) { problems.push("It contains an installer program. Nexus may hold it for a virus scan and managers will not run it.".into()); }
    Ok(Check { files: entries.len(), plan, problems, tips })
}

/// A .zip with each file at its game path (archive/pc/mod/…, r6/scripts/…). Only folders can be packed.
pub fn package(folder: &Path, out_dir: &Path, name: &str, version: &str) -> Result<PathBuf> {
    if !folder.is_dir() { return Err(Error::Invalid("Choose the mod's folder to pack it.".into())); }
    let c = check(folder, name)?;
    if c.plan.files.is_empty() { return Err(Error::Invalid("Nothing in that folder installs into the game.".into())); }
    fs::create_dir_all(out_dir)?;
    let file = out_dir.join(format!("{}-{}.zip", crate::cyberpunk::layout::safe_folder(name).replace(' ', "_"), version.replace(' ', "")));
    let mut z = zip::ZipWriter::new(fs::File::create(&file)?);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for p in &c.plan.files {
        z.start_file(p.target.clone(), opts).map_err(|e| Error::Archive(e.to_string()))?;
        z.write_all(&fs::read(folder.join(&p.source))?)?;
    }
    z.finish().map_err(|e| Error::Archive(e.to_string()))?;
    Ok(file)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Publish {
    pub file: PathBuf,
    /// The Nexus file to add a version to (mod page, Files tab, "Advanced").
    pub file_id: u64,
    pub mod_id: Option<u64>,
    pub version: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    /// main, optional or miscellaneous
    pub category: String,
    pub changelog: Option<String>,
    pub archive_existing: bool,
    pub update_mod_version: bool,
}

/// Upload a new version of a file. `progress(sent, total)` follows the parts.
pub async fn publish(http: &reqwest::Client, key: &str, p: &Publish, mut progress: impl FnMut(u64, u64)) -> Result<u64> {
    let size = fs::metadata(&p.file)?.len();
    let fname = p.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "mod.zip".into());
    let api = |m: reqwest::Method, path: &str| http.request(m, format!("{UPLOAD_BASE}{path}")).header("apikey", key)
        .header("Application-Name", crate::APP_NAME).header("Application-Version", crate::APP_VERSION);
    let fail = |what: &str, status: u16, text: String| Error::Http { status, message: format!("{what}: {}", text.chars().take(300).collect::<String>()) };

    let r = api(reqwest::Method::POST, "/uploads/multipart").json(&serde_json::json!({ "filename": fname, "size_bytes": size.to_string() })).send().await?;
    let st = r.status().as_u16();
    if !r.status().is_success() { return Err(fail("Nexus did not start the upload", st, r.text().await.unwrap_or_default())); }
    let v: serde_json::Value = r.json().await?;
    let d = &v["data"];
    let id = d["id"].as_str().map(str::to_owned).or_else(|| d["id"].as_u64().map(|n| n.to_string())).ok_or_else(|| Error::Decode("no upload id".into()))?;
    let part = d["part_size_bytes"].as_u64().unwrap_or(size.max(1));
    let urls: Vec<String> = d["part_presigned_urls"].as_array().into_iter().flatten().filter_map(|u| u.as_str().map(str::to_owned)).collect();
    let complete = d["complete_presigned_url"].as_str().unwrap_or("").to_string();

    let data = tokio::fs::read(&p.file).await?;
    let mut etags = Vec::new();
    for (i, url) in urls.iter().enumerate() {
        let from = (i as u64 * part) as usize;
        let to = ((i as u64 + 1) * part).min(size) as usize;
        let r = http.put(url).header("Content-Type", "application/octet-stream").body(data[from..to].to_vec()).send().await?;
        let st = r.status().as_u16();
        if !r.status().is_success() { return Err(fail(&format!("Part {} did not upload", i + 1), st, r.text().await.unwrap_or_default())); }
        etags.push(r.headers().get("ETag").and_then(|e| e.to_str().ok()).unwrap_or("").replace('"', ""));
        progress(to as u64, size);
    }
    let xml = format!("<CompleteMultipartUpload>\n{}\n</CompleteMultipartUpload>",
        etags.iter().enumerate().map(|(i, e)| format!("  <Part>\n    <PartNumber>{}</PartNumber>\n    <ETag>{e}</ETag>\n  </Part>", i + 1)).collect::<Vec<_>>().join("\n"));
    let r = http.post(&complete).header("Content-Type", "application/xml").body(xml).send().await?;
    let st = r.status().as_u16();
    if !r.status().is_success() { return Err(fail("The upload did not complete", st, r.text().await.unwrap_or_default())); }
    let r = api(reqwest::Method::POST, &format!("/uploads/{id}/finalise")).send().await?;
    let st = r.status().as_u16();
    if !r.status().is_success() { return Err(fail("Nexus did not accept the upload", st, r.text().await.unwrap_or_default())); }
    // Nexus scans the file; wait for it, backing off
    let mut wait = 2000u64;
    loop {
        let r = api(reqwest::Method::GET, &format!("/uploads/{id}")).send().await?;
        let v: serde_json::Value = r.json().await.unwrap_or_default();
        if v["data"]["state"] == "available" { break; }
        if wait > 120_000 { return Err(Error::Invalid("Nexus is still processing the upload. Check the mod page in a few minutes.".into())); }
        tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
        wait = (wait as f64 * 1.5) as u64;
    }
    let body = serde_json::json!({ "upload_id": id, "name": p.display_name.clone().unwrap_or(fname), "description": p.description, "version": p.version,
        "file_category": p.category, "archive_existing_file": p.archive_existing, "update_mod_version": p.update_mod_version });
    let r = api(reqwest::Method::POST, &format!("/mod-files/{}/versions", p.file_id)).json(&body).send().await?;
    let st = r.status().as_u16();
    if !r.status().is_success() { return Err(fail("Nexus did not add the version", st, r.text().await.unwrap_or_default())); }
    let v: serde_json::Value = r.json().await?;
    let version_id = v["data"]["version"]["id"].as_u64().unwrap_or(0);
    if let (Some(text), Some(mod_id)) = (p.changelog.as_deref().filter(|t| !t.trim().is_empty()), p.mod_id) {
        let _ = api(reqwest::Method::POST, &format!("/mods/{mod_id}/changelogs")).json(&serde_json::json!({ "version": p.version, "changelog": text })).send().await?;
    }
    Ok(version_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checks_and_packs_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let m = dir.path().join("My Mod");
        for (p, b) in [("My Mod/jacket.archive", "a"), ("My Mod/r6/scripts/My/x.reds", "b"), ("My Mod/tool.exe", "c"), ("My Mod/readme.txt", "d")] {
            let f = dir.path().join(p);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, b).unwrap();
        }
        let c = check(&m, "My Mod").unwrap();
        assert!(c.problems.iter().any(|p| p.contains("installer program")));
        assert!(c.tips.iter().any(|t| t.contains("archive/pc/mod")));
        let out = package(&m, &dir.path().join("out"), "My Mod", "1.2").unwrap();
        let names = archive::list(&out).unwrap();
        assert!(names.contains(&"archive/pc/mod/jacket.archive".to_string()) && names.contains(&"r6/scripts/My/x.reds".to_string()));
        assert!(!names.iter().any(|n| n.ends_with(".exe") || n.ends_with(".txt")));
    }
}
