//! Downloading one file to disk with progress and an MD5 (Nexus collections pin archives by MD5).
//! The file is written as `<name>.part` and renamed when complete, so a half download is never mistaken for a whole one.

use crate::{Error, Result};
use futures_util::StreamExt;
use md5::{Digest, Md5};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Downloaded {
    pub path: PathBuf,
    pub size: u64,
    pub md5: String,
}

/// The file name to save as: the URL's last path segment, without its query string.
pub fn file_name_from_url(url: &str, fallback: &str) -> String {
    let name = url::Url::parse(url)
        .ok()
        .and_then(|u| u.path_segments().and_then(|mut s| s.next_back().map(String::from)))
        .map(|s| percent_decode(&s))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback.to_string());
    sanitize(&name)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            out.push(hex(b[i + 1]) * 16 + hex(b[i + 2]));
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn sanitize(name: &str) -> String {
    let s: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let s = s.trim().trim_end_matches('.').to_string();
    if s.is_empty() {
        "download".into()
    } else {
        s
    }
}

/// Streams `url` into `dir`. `on_progress(received, total)` is called as bytes arrive.
pub async fn download(http: &reqwest::Client, url: &str, dir: &Path, name: &str, mut on_progress: impl FnMut(u64, Option<u64>)) -> Result<Downloaded> {
    tokio::fs::create_dir_all(dir).await.map_err(|e| Error::io_at(e, dir))?;
    let path = dir.join(sanitize(name));
    let part = path.with_file_name(format!("{}.part", path.file_name().unwrap_or_default().to_string_lossy()));
    let resp = http.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(Error::Network(format!("The download server answered {}.", resp.status())));
    }
    let total = resp.content_length();
    let mut file = tokio::fs::File::create(&part).await.map_err(|e| Error::io_at(e, &part))?;
    let mut hasher = Md5::new();
    let mut received = 0u64;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(|e| Error::io_at(e, &part))?;
        received += chunk.len() as u64;
        on_progress(received, total);
    }
    file.flush().await?;
    drop(file);
    if let Some(t) = total {
        if t != received {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(Error::Network(format!("The download stopped early ({received} of {t} bytes).")));
        }
    }
    tokio::fs::rename(&part, &path).await.map_err(|e| Error::io_at(e, &path))?;
    Ok(Downloaded { path, size: received, md5: hex::encode(hasher.finalize()) })
}

/// MD5 of a file already on disk.
pub fn md5_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| Error::io_at(e, path))?;
    let mut h = Md5::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_urls() {
        assert_eq!(file_name_from_url("https://cf-files.nexusmods.com/cdn/3333/107/CET%201.32.zip?md5=x&expires=1", "f"), "CET 1.32.zip");
        assert_eq!(file_name_from_url("https://x/", "fallback.7z"), "fallback.7z");
        assert_eq!(sanitize("a/b:c?.zip"), "a_b_c_.zip");
        assert_eq!(sanitize("..."), "download");
    }

    #[test]
    fn md5_of_known_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x");
        std::fs::write(&p, b"hello").unwrap();
        assert_eq!(md5_file(&p).unwrap(), "5d41402abc4b2a76b9719d911017c592");
    }

    #[tokio::test]
    async fn downloads_with_progress_and_md5() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf).await;
            s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\nconnection: close\r\n\r\nhello").await.unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let mut calls = 0;
        let d = download(&reqwest::Client::new(), &format!("http://{addr}/f.zip"), dir.path(), "f.zip", |r, t| {
            calls += 1;
            assert_eq!(t, Some(5));
            assert!(r <= 5);
        })
        .await
        .unwrap();
        assert!(calls >= 1);
        assert_eq!(d.size, 5);
        assert_eq!(d.md5, "5d41402abc4b2a76b9719d911017c592");
        assert_eq!(std::fs::read(&d.path).unwrap(), b"hello");
        assert!(!dir.path().join("f.zip.part").exists());
    }
}
