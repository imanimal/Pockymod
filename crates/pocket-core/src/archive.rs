//! Listing and extracting .zip, .7z and .rar archives with bundled libraries (no 7-Zip install needed).
//!
//! Every entry path is checked before anything is written, so an archive can't place files outside the
//! destination folder (absolute paths, drive letters and `..` are refused).

use crate::{Error, Result};
use serde::Serialize;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Zip,
    SevenZ,
    Rar,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Path inside the archive, always with forward slashes.
    pub path: String,
    pub size: u64,
}

/// Identifies the format from the first bytes, not the file name.
pub fn detect(path: &Path) -> Result<Format> {
    let mut head = [0u8; 8];
    let mut f = File::open(path).map_err(|e| Error::io_at(e, path))?;
    let n = f.read(&mut head)?;
    let head = &head[..n];
    if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
        Ok(Format::Zip)
    } else if head.starts_with(&[b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C]) {
        Ok(Format::SevenZ)
    } else if head.starts_with(b"Rar!\x1A\x07") {
        Ok(Format::Rar)
    } else {
        Err(Error::Archive(format!("{} is not a zip, 7z or rar archive.", path.display())))
    }
}

/// A relative path that is safe to join onto a destination, or None.
pub fn safe_rel(raw: &str) -> Option<PathBuf> {
    let raw = raw.replace('\\', "/");
    if raw.starts_with('/') || raw.contains(':') {
        return None;
    }
    let mut out = PathBuf::new();
    for c in Path::new(&raw).components() {
        match c {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn rel_string(p: &Path) -> String {
    p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")
}

/// Lists the files (not folders) in an archive.
pub fn list(path: &Path) -> Result<Vec<Entry>> {
    match detect(path)? {
        Format::Zip => {
            let mut z = zip::ZipArchive::new(File::open(path)?).map_err(|e| Error::Archive(e.to_string()))?;
            let mut out = Vec::new();
            for i in 0..z.len() {
                let f = z.by_index(i).map_err(|e| Error::Archive(e.to_string()))?;
                if f.is_dir() {
                    continue;
                }
                if let Some(p) = f.enclosed_name() {
                    out.push(Entry { path: rel_string(&p), size: f.size() });
                }
            }
            Ok(out)
        }
        Format::SevenZ => {
            let r = sevenz_rust2::ArchiveReader::open(path, sevenz_rust2::Password::empty())
                .map_err(|e| Error::Archive(e.to_string()))?;
            Ok(r.archive()
                .files
                .iter()
                .filter(|e| !e.is_directory())
                .filter_map(|e| safe_rel(e.name()).map(|p| Entry { path: rel_string(&p), size: e.size() }))
                .collect())
        }
        Format::Rar => {
            let mut out = Vec::new();
            let open = unrar::Archive::new(path).open_for_listing().map_err(|e| Error::Archive(e.to_string()))?;
            for e in open {
                let e = e.map_err(|e| Error::Archive(e.to_string()))?;
                if e.is_file() {
                    if let Some(p) = safe_rel(&e.filename.to_string_lossy()) {
                        out.push(Entry { path: rel_string(&p), size: e.unpacked_size });
                    }
                }
            }
            Ok(out)
        }
    }
}

/// Extracts every file into `dest` (created if needed). Returns the relative paths written.
pub fn extract(path: &Path, dest: &Path) -> Result<Vec<String>> {
    fs::create_dir_all(dest).map_err(|e| Error::io_at(e, dest))?;
    let mut written = Vec::new();
    match detect(path)? {
        Format::Zip => {
            let mut z = zip::ZipArchive::new(File::open(path)?).map_err(|e| Error::Archive(e.to_string()))?;
            for i in 0..z.len() {
                let mut f = z.by_index(i).map_err(|e| Error::Archive(e.to_string()))?;
                if f.is_dir() {
                    continue;
                }
                let Some(rel) = f.enclosed_name() else { continue };
                let out = dest.join(&rel);
                write_entry(&out, &mut f)?;
                written.push(rel_string(&rel));
            }
        }
        Format::SevenZ => {
            let mut r = sevenz_rust2::ArchiveReader::open(path, sevenz_rust2::Password::empty())
                .map_err(|e| Error::Archive(e.to_string()))?;
            let mut failure: Option<Error> = None;
            r.for_each_entries(|entry: &sevenz_rust2::ArchiveEntry, reader: &mut dyn Read| {
                if entry.is_directory() {
                    return Ok(true);
                }
                let Some(rel) = safe_rel(entry.name()) else { return Ok(true) };
                let out = dest.join(&rel);
                if let Err(e) = write_entry(&out, reader) {
                    failure = Some(e);
                    return Ok(false);
                }
                written.push(rel_string(&rel));
                Ok(true)
            })
            .map_err(|e| Error::Archive(e.to_string()))?;
            if let Some(e) = failure {
                return Err(e);
            }
        }
        Format::Rar => {
            let mut open = unrar::Archive::new(path).open_for_processing().map_err(|e| Error::Archive(e.to_string()))?;
            while let Some(header) = open.read_header().map_err(|e| Error::Archive(e.to_string()))? {
                let entry = header.entry();
                let rel = if entry.is_file() { safe_rel(&entry.filename.to_string_lossy()) } else { None };
                open = match rel {
                    Some(rel) => {
                        let out = dest.join(&rel);
                        if let Some(parent) = out.parent() {
                            fs::create_dir_all(parent).map_err(|e| Error::io_at(e, parent))?;
                        }
                        let next = header.extract_to(&out).map_err(|e| Error::Archive(e.to_string()))?;
                        written.push(rel_string(&rel));
                        next
                    }
                    None => header.skip().map_err(|e| Error::Archive(e.to_string()))?,
                };
            }
        }
    }
    Ok(written)
}

fn write_entry(out: &Path, reader: &mut dyn Read) -> Result<()> {
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io_at(e, parent))?;
    }
    let mut file = File::create(out).map_err(|e| Error::io_at(e, out))?;
    io::copy(reader, &mut file).map_err(|e| Error::io_at(e, out))?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    pub fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(File::create(path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in files {
            z.start_file(*name, opts).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn safe_rel_refuses_escapes() {
        assert_eq!(safe_rel("a/b.txt"), Some(PathBuf::from("a").join("b.txt")));
        assert_eq!(safe_rel("a\\b.txt"), Some(PathBuf::from("a").join("b.txt")));
        assert_eq!(safe_rel("./a"), Some(PathBuf::from("a")));
        assert_eq!(safe_rel("../evil"), None);
        assert_eq!(safe_rel("a/../../evil"), None);
        assert_eq!(safe_rel("/etc/passwd"), None);
        assert_eq!(safe_rel("C:\\Windows\\x"), None);
        assert_eq!(safe_rel(""), None);
    }

    #[test]
    fn zip_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let zp = dir.path().join("m.zip");
        make_zip(&zp, &[("Mod/archive/pc/mod/a.archive", b"AAAA"), ("Mod/readme.txt", b"hi")]);
        assert_eq!(detect(&zp).unwrap(), Format::Zip);
        let l = list(&zp).unwrap();
        assert_eq!(l.len(), 2);
        assert!(l.iter().any(|e| e.path == "Mod/archive/pc/mod/a.archive" && e.size == 4));
        let out = dir.path().join("out");
        let w = extract(&zp, &out).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(fs::read(out.join("Mod/archive/pc/mod/a.archive")).unwrap(), b"AAAA");
    }

    #[test]
    fn sevenz_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(src.join("r6/scripts/m")).unwrap();
        fs::write(src.join("r6/scripts/m/m.reds"), b"// hi").unwrap();
        let ap = dir.path().join("m.7z");
        sevenz_rust2::compress_to_path(&src, &ap).unwrap();
        assert_eq!(detect(&ap).unwrap(), Format::SevenZ);
        let l = list(&ap).unwrap();
        assert!(l.iter().any(|e| e.path.ends_with("r6/scripts/m/m.reds")), "{l:?}");
        let out = dir.path().join("out");
        let w = extract(&ap, &out).unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(fs::read(out.join(&w[0])).unwrap(), b"// hi");
    }

    #[test]
    fn rejects_unknown_format() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.zip");
        fs::write(&p, b"hello").unwrap();
        assert!(matches!(detect(&p), Err(Error::Archive(_))));
    }
}
