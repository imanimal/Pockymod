//! Download a file from a Nexus CDN link to the library's downloads folder, hashing it on the way (MD5, as Nexus
//! lists file hashes). Written to `<name>.part` and renamed when complete, so a broken download never looks finished.

use crate::error::{Error, Result};
use futures_util::StreamExt;
use md5::{Digest, Md5};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

pub const CANCELLED: &str = "Download cancelled. Retry picks it up where it stopped.";

pub struct Downloaded { pub path: PathBuf, pub size: u64, pub md5: String }

/// A file name safe on Windows, from Nexus's `uri` (e.g. `ArchiveXL-4198-1-28-6-1761742782.zip`).
pub fn safe_file_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let s = s.trim().trim_end_matches(['.', ' ']).to_string();
    if s.is_empty() { "download.bin".into() } else { s }
}

/// `progress(received, total)` is called as bytes arrive. A connection that drops is picked up again where it
/// stopped (a Range request), up to five times with a growing pause, so a long download on a free account survives
/// a hiccup without a new click on Nexus.
pub async fn download(http: &reqwest::Client, url: &str, dir: &Path, file_name: &str, mut progress: impl FnMut(u64, Option<u64>) -> bool) -> Result<Downloaded> {
    tokio::fs::create_dir_all(dir).await?;
    let path = dir.join(safe_file_name(file_name));
    let part = path.with_extension(format!("{}part", path.extension().map(|e| format!("{}.", e.to_string_lossy())).unwrap_or_default()));
    let mut attempt = 0u32;
    loop {
        match try_download(http, url, &part, &mut progress).await {
            Ok(()) => break,
            Err(e @ Error::Network(_)) if attempt < 5 => {
                attempt += 1;
                tracing_pause(attempt).await;
                let _ = e;
            }
            Err(e) => return Err(e),
        }
    }
    // hash the finished file in one pass (it may have arrived in pieces)
    let (size, md5) = hash_file(&part).await?;
    tokio::fs::rename(&part, &path).await?;
    Ok(Downloaded { path, size, md5 })
}

async fn tracing_pause(attempt: u32) { tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt.min(4)))).await; }

async fn hash_file(p: &Path) -> Result<(u64, String)> {
    let mut f = tokio::fs::File::open(p).await?;
    let mut hash = Md5::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut n_all = 0u64;
    loop {
        let n = tokio::io::AsyncReadExt::read(&mut f, &mut buf).await?;
        if n == 0 { break; }
        hash.update(&buf[..n]);
        n_all += n as u64;
    }
    Ok((n_all, hex::encode(hash.finalize())))
}

/// One request: from the start, or the rest of a `.part` left by an earlier try.
async fn try_download(http: &reqwest::Client, url: &str, part: &Path, progress: &mut impl FnMut(u64, Option<u64>) -> bool) -> Result<()> {
    let have = tokio::fs::metadata(part).await.map(|m| m.len()).unwrap_or(0);
    let mut req = http.get(url);
    if have > 0 { req = req.header("Range", format!("bytes={have}-")); }
    let resp = req.send().await.map_err(|e| Error::Network(format!("{}. Retry to resume the download.", e.without_url())))?;
    // asked for the rest of a file that is already complete
    if have > 0 && resp.status().as_u16() == 416 { progress(have, Some(have)); return Ok(()); }
    if !resp.status().is_success() {
        return Err(Error::Http { status: resp.status().as_u16(), message: "The download server refused the link. Links from Nexus expire; try the file again.".into() });
    }
    let resumed = have > 0 && resp.status().as_u16() == 206;
    let mut received = if resumed { have } else { 0 };
    let mut file = if resumed { tokio::fs::OpenOptions::new().append(true).open(part).await? } else { tokio::fs::File::create(part).await? };
    let total = resp.content_length().map(|l| l + received);
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| Error::Network(format!("{}. Retry to resume the download.", e.without_url())))?;
        file.write_all(&chunk).await?;
        received += chunk.len() as u64;
        // false: the user cancelled; the part stays, so starting it again resumes
        if !progress(received, total) { file.flush().await?; return Err(Error::Invalid(CANCELLED.into())); }
    }
    file.flush().await?;
    drop(file);
    if total.is_some_and(|t| t != received) {
        // keep the part: the next try asks for the rest
        return Err(Error::Network(format!("The download stopped early ({received} of {} bytes). Retry to resume it.", total.unwrap_or(0))));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};

    /// A one-file HTTP server that honours Range requests.
    fn serve(body: &'static [u8]) -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut from = 0usize;
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" { break; }
                    if let Some(v) = line.to_lowercase().strip_prefix("range: bytes=") { from = v.trim().trim_end_matches('-').parse().unwrap_or(0); }
                }
                let mut s = s;
                let part = &body[from..];
                let status = if from > 0 { "206 Partial Content" } else { "200 OK" };
                let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", part.len());
                let _ = s.write_all(part);
            }
        });
        format!("http://{addr}/file.zip")
    }

    #[tokio::test]
    async fn resumes_a_part_file_and_hashes_the_whole() {
        let body: &'static [u8] = b"hello world, this is a mod archive";
        let url = serve(body);
        let dir = tempfile::tempdir().unwrap();
        // half of it is already there from an earlier try
        std::fs::write(dir.path().join("m.zip.part"), &body[..10]).unwrap();
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let d = download(&http, &url, dir.path(), "m.zip", |_, _| true).await.unwrap();
        assert_eq!(std::fs::read(&d.path).unwrap(), body);
        assert_eq!(d.md5, hex::encode(Md5::digest(body)));
        assert!(!dir.path().join("m.zip.part").exists());
    }
    #[test]
    fn names() {
        assert_eq!(safe_file_name("ArchiveXL-4198-1-28-6.zip"), "ArchiveXL-4198-1-28-6.zip");
        assert_eq!(safe_file_name("a/b:c?.7z"), "a_b_c_.7z");
        assert_eq!(safe_file_name(" . "), "download.bin");
    }
}
