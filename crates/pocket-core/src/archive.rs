//! Archive listing and extraction: .zip (zip), .7z (sevenz-rust2) and .rar (unrar, the bundled UnRAR source).
//! Every entry path is checked before anything is written: no absolute paths, drive letters or `..`, so an archive
//! cannot write outside its staging folder. Links and devices are skipped; only regular files are extracted.

use crate::error::{Error, Result};
use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format { Zip, SevenZ, Rar }

/// By content first (magic bytes), then by extension.
pub fn detect(path: &Path) -> Result<Format> {
    let mut head = [0u8; 8];
    let n = io::Read::read(&mut File::open(path)?, &mut head)?;
    let h = &head[..n];
    if h.starts_with(b"PK\x03\x04") || h.starts_with(b"PK\x05\x06") { return Ok(Format::Zip); }
    if h.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) { return Ok(Format::SevenZ); }
    if h.starts_with(b"Rar!\x1a\x07") { return Ok(Format::Rar); }
    match path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).as_deref() {
        Some("zip") => Ok(Format::Zip),
        Some("7z") => Ok(Format::SevenZ),
        Some("rar") => Ok(Format::Rar),
        _ => Err(Error::Archive("This file is not a .zip, .7z or .rar archive.".into())),
    }
}

/// A relative path that stays inside its base, with forward slashes; None for anything unsafe.
pub fn safe_relative(name: &str) -> Option<PathBuf> {
    let name = name.replace('\\', "/");
    if name.starts_with('/') || name.contains('\0') { return None; }
    let mut out = PathBuf::new();
    for c in Path::new(&name).components() {
        match c {
            Component::Normal(p) => {
                let s = p.to_string_lossy();
                // a drive letter or alternate data stream (C:, file:stream) has no place in an archive path
                if s.contains(':') { return None; }
                out.push(p);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn rel_string(p: &Path) -> String { p.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/") }

/// The file paths in an archive (directories left out), without extracting.
pub fn list(path: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    match detect(path)? {
        Format::Zip => {
            let mut z = zip::ZipArchive::new(File::open(path)?).map_err(|e| Error::Archive(e.to_string()))?;
            for i in 0..z.len() {
                let f = z.by_index_raw(i).map_err(|e| Error::Archive(e.to_string()))?;
                if f.is_file() { if let Some(p) = safe_relative(f.name()) { out.push(rel_string(&p)); } }
            }
        }
        Format::SevenZ => {
            let a = sevenz_rust2::Archive::open(path).map_err(|e| Error::Archive(e.to_string()))?;
            for e in &a.files {
                if !e.is_directory() && e.has_stream() { if let Some(p) = safe_relative(e.name()) { out.push(rel_string(&p)); } }
            }
        }
        Format::Rar => {
            let arch = unrar::Archive::new(path).open_for_listing().map_err(|e| Error::Archive(e.to_string()))?;
            for e in arch {
                let e = e.map_err(|e| Error::Archive(e.to_string()))?;
                if e.is_file() { if let Some(p) = safe_relative(&e.filename.to_string_lossy()) { out.push(rel_string(&p)); } }
            }
        }
    }
    Ok(out)
}

/// Extract every regular file into `dest` (created if needed). Returns the relative paths written.
pub fn extract(path: &Path, dest: &Path) -> Result<Vec<String>> {
    fs::create_dir_all(dest)?;
    let mut written = Vec::new();
    let target = |rel: &Path| -> Result<PathBuf> {
        let t = dest.join(rel);
        if let Some(parent) = t.parent() { fs::create_dir_all(parent)?; }
        Ok(t)
    };
    match detect(path)? {
        Format::Zip => {
            let mut z = zip::ZipArchive::new(File::open(path)?).map_err(|e| Error::Archive(e.to_string()))?;
            for i in 0..z.len() {
                let mut f = z.by_index(i).map_err(|e| Error::Archive(e.to_string()))?;
                if !f.is_file() { continue; }
                let Some(rel) = safe_relative(f.name()) else { continue };
                let mut out = File::create(target(&rel)?)?;
                io::copy(&mut f, &mut out)?;
                written.push(rel_string(&rel));
            }
        }
        Format::SevenZ => {
            let mut r = sevenz_rust2::ArchiveReader::open(path, sevenz_rust2::Password::empty()).map_err(|e| Error::Archive(e.to_string()))?;
            let mut err: Option<Error> = None;
            r.for_each_entries(|entry, reader| {
                if entry.is_directory() || !entry.has_stream() { return Ok(true); }
                let Some(rel) = safe_relative(entry.name()) else {
                    // solid archives must still be read through
                    io::copy(reader, &mut io::sink())?;
                    return Ok(true);
                };
                match target(&rel).and_then(|t| File::create(t).map_err(Error::from)) {
                    Ok(mut out) => { io::copy(reader, &mut out)?; written.push(rel_string(&rel)); Ok(true) }
                    Err(e) => { err = Some(e); Ok(false) }
                }
            }).map_err(|e| Error::Archive(e.to_string()))?;
            if let Some(e) = err { return Err(e); }
        }
        Format::Rar => {
            let mut arch = unrar::Archive::new(path).open_for_processing().map_err(|e| Error::Archive(e.to_string()))?;
            while let Some(header) = arch.read_header().map_err(|e| Error::Archive(e.to_string()))? {
                let entry = header.entry();
                let rel = entry.is_file().then(|| safe_relative(&entry.filename.to_string_lossy())).flatten();
                arch = match rel {
                    Some(rel) => {
                        let t = target(&rel)?;
                        written.push(rel_string(&rel));
                        header.extract_to(t).map_err(|e| Error::Archive(e.to_string()))?
                    }
                    None => header.skip().map_err(|e| Error::Archive(e.to_string()))?,
                };
            }
        }
    }
    Ok(written)
}

/// MD5 of a file, as Nexus uses for file hashes. Hex, lower case.
pub fn md5_file(path: &Path) -> Result<String> {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    let mut f = File::open(path)?;
    io::copy(&mut f, &mut h)?;
    Ok(hex::encode(h.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn unsafe_paths_are_refused() {
        assert_eq!(safe_relative("a/b.archive"), Some(PathBuf::from("a/b.archive")));
        assert_eq!(safe_relative("a\\b\\c.reds").map(|p| rel_string(&p)), Some("a/b/c.reds".into()));
        assert_eq!(safe_relative("./x"), Some(PathBuf::from("x")));
        for bad in ["../x", "a/../../x", "/etc/passwd", "C:/Windows/x", "C:\\x", "a/file:stream", ""] {
            assert!(safe_relative(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn zip_roundtrip_skips_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let zp = dir.path().join("m.zip");
        {
            let mut z = zip::ZipWriter::new(File::create(&zp).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            z.add_directory("MyMod/", o).unwrap();
            z.start_file("MyMod/archive/pc/mod/a.archive", o).unwrap();
            z.write_all(b"hello").unwrap();
            z.start_file("../evil.txt", o).unwrap();
            z.write_all(b"no").unwrap();
            z.finish().unwrap();
        }
        assert_eq!(list(&zp).unwrap(), ["MyMod/archive/pc/mod/a.archive"]);
        let out = dir.path().join("out");
        assert_eq!(extract(&zp, &out).unwrap(), ["MyMod/archive/pc/mod/a.archive"]);
        assert_eq!(fs::read(out.join("MyMod/archive/pc/mod/a.archive")).unwrap(), b"hello");
        assert!(!dir.path().join("evil.txt").exists());
        assert_eq!(md5_file(&out.join("MyMod/archive/pc/mod/a.archive")).unwrap(), "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn sevenz_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(src.join("r6/scripts/X")).unwrap();
        fs::write(src.join("r6/scripts/X/a.reds"), b"func x() {}").unwrap();
        let p = dir.path().join("m.7z");
        sevenz_rust2::compress_to_path(&src, &p).unwrap();
        assert_eq!(detect(&p).unwrap(), Format::SevenZ);
        assert_eq!(list(&p).unwrap(), ["r6/scripts/X/a.reds"]);
        let out = dir.path().join("out");
        extract(&p, &out).unwrap();
        assert_eq!(fs::read(out.join("r6/scripts/X/a.reds")).unwrap(), b"func x() {}");
    }

    #[test]
    fn not_an_archive() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.bin");
        fs::write(&p, b"nope").unwrap();
        assert!(matches!(detect(&p), Err(Error::Archive(_))));
    }
}
