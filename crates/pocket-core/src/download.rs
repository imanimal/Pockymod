//! Download a file from a Nexus CDN link to the library's downloads folder, hashing it on the way (MD5, as Nexus
//! lists file hashes). Written to `<name>.part` and renamed when complete, so a broken download never looks finished.

use crate::error::{Error, Result};
use futures_util::StreamExt;
use md5::{Digest, Md5};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

pub struct Downloaded { pub path: PathBuf, pub size: u64, pub md5: String }

/// A file name safe on Windows, from Nexus's `uri` (e.g. `ArchiveXL-4198-1-28-6-1761742782.zip`).
pub fn safe_file_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let s = s.trim().trim_end_matches(['.', ' ']).to_string();
    if s.is_empty() { "download.bin".into() } else { s }
}

/// `progress(received, total)` is called as bytes arrive.
pub async fn download(http: &reqwest::Client, url: &str, dir: &Path, file_name: &str, mut progress: impl FnMut(u64, Option<u64>)) -> Result<Downloaded> {
    tokio::fs::create_dir_all(dir).await?;
    let path = dir.join(safe_file_name(file_name));
    let part = path.with_extension(format!("{}part", path.extension().map(|e| format!("{}.", e.to_string_lossy())).unwrap_or_default()));
    let resp = http.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(Error::Http { status: resp.status().as_u16(), message: "The download server refused the link. Links from Nexus expire; try the file again.".into() });
    }
    let total = resp.content_length();
    let mut file = tokio::fs::File::create(&part).await?;
    let mut hash = Md5::new();
    let mut received = 0u64;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| Error::Network(e.without_url().to_string()))?;
        hash.update(&chunk);
        file.write_all(&chunk).await?;
        received += chunk.len() as u64;
        progress(received, total);
    }
    file.flush().await?;
    drop(file);
    if total.is_some_and(|t| t != received) {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(Error::Network(format!("The download stopped early ({received} of {} bytes).", total.unwrap_or(0))));
    }
    tokio::fs::rename(&part, &path).await?;
    Ok(Downloaded { path, size: received, md5: hex::encode(hash.finalize()) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert_eq!(safe_file_name("ArchiveXL-4198-1-28-6.zip"), "ArchiveXL-4198-1-28-6.zip");
        assert_eq!(safe_file_name("a/b:c?.7z"), "a_b_c_.7z");
        assert_eq!(safe_file_name(" . "), "download.bin");
    }
}
